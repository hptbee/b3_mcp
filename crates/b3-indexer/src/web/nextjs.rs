use super::*;

pub(super) fn collect_nextjs_routes(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let Some(route_file) = nextjs_route_file(&input.path) else {
        return Vec::new();
    };
    match route_file {
        NextjsRouteFile::AppRouteHandler { path } => {
            nextjs_app_route_handler_symbols(root, input, symbols, &path)
        }
        NextjsRouteFile::AppSpecial {
            path,
            kind,
            source_kind,
        } => {
            vec![nextjs_file_route_symbol(input, &path, &kind, source_kind)]
        }
        NextjsRouteFile::PagesPage { path } => {
            vec![nextjs_file_route_symbol(
                input,
                &path,
                "page",
                "NextPagesPage",
            )]
        }
        NextjsRouteFile::PagesApi { path } => {
            vec![nextjs_file_route_symbol(
                input,
                &path,
                "api",
                "NextPagesApiRoute",
            )]
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NextjsRouteFile {
    AppSpecial {
        path: String,
        kind: String,
        source_kind: &'static str,
    },
    AppRouteHandler {
        path: String,
    },
    PagesPage {
        path: String,
    },
    PagesApi {
        path: String,
    },
}

fn nextjs_route_file(path: &Path) -> Option<NextjsRouteFile> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if let Some(index) = parts.iter().rposition(|part| *part == "app") {
        return nextjs_app_route_file(&parts[index + 1..]);
    }
    if let Some(index) = parts.iter().rposition(|part| *part == "pages") {
        return nextjs_pages_route_file(&parts[index + 1..]);
    }
    None
}

fn nextjs_app_route_file(parts: &[&str]) -> Option<NextjsRouteFile> {
    let file_name = parts.last()?;
    let stem = nextjs_file_stem(file_name)?;
    if stem == "route" {
        let route_path = nextjs_route_path_from_segments(&parts[..parts.len().saturating_sub(1)])?;
        return Some(NextjsRouteFile::AppRouteHandler { path: route_path });
    }
    let (kind, source_kind) = match stem {
        "page" => ("page", "NextAppPage"),
        "layout" => ("layout", "NextAppLayout"),
        "loading" => ("loading", "NextAppLoading"),
        "error" => ("error", "NextAppError"),
        "not-found" => ("not_found", "NextAppNotFound"),
        "template" => ("template", "NextAppTemplate"),
        _ => return None,
    };
    let route_path = nextjs_route_path_from_segments(&parts[..parts.len().saturating_sub(1)])?;
    Some(NextjsRouteFile::AppSpecial {
        path: route_path,
        kind: kind.to_string(),
        source_kind,
    })
}

fn nextjs_pages_route_file(parts: &[&str]) -> Option<NextjsRouteFile> {
    let file_name = parts.last()?;
    let stem = nextjs_file_stem(file_name)?;
    if stem.starts_with('_') {
        return None;
    }
    let mut route_parts = parts[..parts.len().saturating_sub(1)].to_vec();
    if stem != "index" {
        route_parts.push(stem);
    }
    let route_path = nextjs_route_path_from_segments(&route_parts)?;
    if route_parts.first().copied() == Some("api") {
        Some(NextjsRouteFile::PagesApi { path: route_path })
    } else {
        Some(NextjsRouteFile::PagesPage { path: route_path })
    }
}

fn nextjs_file_stem(file_name: &str) -> Option<&str> {
    for extension in ["tsx", "jsx", "ts", "js", "mjs", "cjs"] {
        let suffix = format!(".{extension}");
        if let Some(stem) = file_name.strip_suffix(&suffix) {
            return Some(stem);
        }
    }
    None
}

fn nextjs_route_path_from_segments(segments: &[&str]) -> Option<String> {
    let mut route_segments = Vec::new();
    for segment in segments {
        if segment.starts_with("(.)") || segment.starts_with("(..)") || segment.starts_with("(...)")
        {
            return None;
        }
        if segment.starts_with('@') || (segment.starts_with('(') && segment.ends_with(')')) {
            continue;
        }
        if *segment == "index" {
            continue;
        }
        route_segments.push(nextjs_route_segment(segment)?);
    }
    Some(if route_segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", route_segments.join("/"))
    })
}

fn nextjs_route_segment(segment: &str) -> Option<String> {
    if segment.starts_with("[[...") && segment.ends_with("]]") {
        let name = segment.trim_start_matches("[[...").trim_end_matches("]]");
        return (!name.is_empty()).then(|| format!("*{name}?"));
    }
    if segment.starts_with("[...") && segment.ends_with(']') {
        let name = segment.trim_start_matches("[...").trim_end_matches(']');
        return (!name.is_empty()).then(|| format!("*{name}"));
    }
    if segment.starts_with('[') && segment.ends_with(']') {
        let name = segment.trim_start_matches('[').trim_end_matches(']');
        return (!name.is_empty()).then(|| format!(":{name}"));
    }
    Some(segment.to_string())
}

fn nextjs_app_route_handler_symbols(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    path: &str,
) -> Vec<ExtractedSymbol> {
    let mut exports = Vec::new();
    collect_nextjs_http_method_exports(root, input, symbols, &mut exports);
    exports
        .into_iter()
        .map(|symbol| {
            nextjs_route_symbol_from_range(
                input,
                &symbol.method,
                path,
                "api",
                "NextAppRouteHandler",
                symbol.handler_name.as_deref(),
                Some(&symbol.function_name),
                symbol.symbol_id,
                symbol.start_byte,
                symbol.end_byte,
                symbol.start_position,
                symbol.end_position,
                9_500,
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
struct NextjsMethodExport {
    method: String,
    function_name: String,
    handler_name: Option<String>,
    symbol_id: Option<SymbolId>,
    start_byte: usize,
    end_byte: usize,
    start_position: Point,
    end_position: Point,
}

fn collect_nextjs_http_method_exports(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    exports: &mut Vec<NextjsMethodExport>,
) {
    if node.kind() == "export_statement" {
        if let Some(export) = nextjs_http_method_export(node, input, symbols) {
            exports.push(export);
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nextjs_http_method_exports(child, input, symbols, exports);
    }
}

fn nextjs_http_method_export(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Option<NextjsMethodExport> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "generator_function_declaration" => {
                let name = child.child_by_field_name("name")?;
                let method = node_text(name, &input.source);
                if NEXTJS_HTTP_METHODS.contains(&method) {
                    return Some(nextjs_method_export_from_node(method, child, symbols));
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                let mut declaration_cursor = child.walk();
                for declarator in child.named_children(&mut declaration_cursor) {
                    if declarator.kind() != "variable_declarator" {
                        continue;
                    }
                    let name = declarator.child_by_field_name("name")?;
                    let method = node_text(name, &input.source);
                    if NEXTJS_HTTP_METHODS.contains(&method) {
                        return Some(nextjs_method_export_from_node(method, declarator, symbols));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn nextjs_method_export_from_node(
    method: &str,
    node: Node<'_>,
    symbols: &[ExtractedSymbol],
) -> NextjsMethodExport {
    let symbol_id = symbols
        .iter()
        .find(|symbol| {
            symbol.name == method
                && symbol.start_byte <= node.start_byte()
                && symbol.end_byte >= node.end_byte()
        })
        .map(|symbol| symbol.id.clone());
    NextjsMethodExport {
        method: method.to_string(),
        function_name: method.to_string(),
        handler_name: Some(method.to_string()),
        symbol_id,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_position: node.start_position(),
        end_position: node.end_position(),
    }
}

fn nextjs_file_route_symbol(
    input: &ParseInput,
    path: &str,
    kind: &str,
    source_kind: &str,
) -> ExtractedSymbol {
    nextjs_route_symbol_from_range(
        input,
        "GET",
        path,
        kind,
        source_kind,
        None,
        None,
        None,
        0,
        input.source.len(),
        Point { row: 0, column: 0 },
        Point {
            row: input.source.lines().count().saturating_sub(1),
            column: input.source.lines().last().unwrap_or_default().len(),
        },
        9_000,
    )
}

fn nextjs_route_symbol_from_range(
    input: &ParseInput,
    method: &str,
    path: &str,
    kind: &str,
    source_kind: &str,
    handler_name: Option<&str>,
    function_name: Option<&str>,
    symbol_id: Option<SymbolId>,
    start_byte: usize,
    end_byte: usize,
    start_position: Point,
    end_position: Point,
    confidence: u16,
) -> ExtractedSymbol {
    let line_start = one_based_row(start_position);
    let line_end = one_based_row(end_position);
    let metadata = RouteMetadata {
        framework: "nextjs".to_string(),
        route_kind: kind.to_string(),
        method: method.to_string(),
        path: path.to_string(),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: symbol_id.clone(),
        handler_name: handler_name.map(str::to_string),
        class_name: None,
        function_name: function_name.map(str::to_string),
        line_start,
        line_end,
        confidence,
        source_kind: source_kind.to_string(),
    };
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:nextjs-route:{}:{}:{}:{}",
                input.file_id.as_str(),
                method,
                path,
                source_kind,
                start_byte
            ),
        )),
        file_id: input.file_id.clone(),
        name: format!("{method} {path}"),
        kind: NodeKind::Route,
        start_byte,
        end_byte,
        start_line: line_start,
        start_column: start_position.column,
        end_line: line_end,
        end_column: end_position.column,
        visibility: Some(encode_route_metadata(&metadata)),
    }
}

pub fn detect_nextjs_config_path(path: &Path) -> Option<DetectedTechnology> {
    let file_name = path.file_name()?.to_str()?;
    if !matches!(
        file_name,
        "next.config.js" | "next.config.mjs" | "next.config.ts"
    ) {
        return None;
    }
    Some(DetectedTechnology {
        id: "nextjs".to_string(),
        name: "Next.js".to_string(),
        kind: TechnologyKind::WebFrontend,
        support_level: TechnologySupportLevel::Basic,
        capabilities: vec![
            TechnologyCapability::DetectPackage,
            TechnologyCapability::DetectImport,
            TechnologyCapability::ExtractRoutes,
            TechnologyCapability::ExtractComponents,
        ],
        source: format!("config:{}", path.to_string_lossy().replace('\\', "/")),
    })
}
