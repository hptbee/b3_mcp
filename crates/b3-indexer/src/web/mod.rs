use super::*;

mod angular;

#[cfg(test)]
pub(crate) use angular::angular_metadata_value;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let Some(language) = language_from_path(&input.path) else {
        return NoopTreeSitterParser.parse(input);
    };
    if !matches!(
        language.as_str(),
        "javascript" | "jsx" | "typescript" | "tsx"
    ) {
        return NoopTreeSitterParser.parse(input);
    }

    let mut parser = Parser::new();
    let tree_sitter_language = match language.as_str() {
        "javascript" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => unreachable!("language checked above"),
    };
    parser
        .set_language(&tree_sitter_language)
        .map_err(to_contract_error)?;
    let tree = parser
        .parse(&input.source, None)
        .ok_or_else(|| ContractError::new("tree-sitter web language parse failed"))?;

    let root = tree.root_node();
    let mut symbols = vec![module_symbol(&input)];
    collect_web_symbols(root, &input, &mut symbols);
    annotate_react_components(root, &input, &mut symbols);
    angular::annotate_angular_symbols(root, &input, &mut symbols);
    let routes = collect_node_rest_routes(root, &input, &symbols);
    symbols.extend(routes);
    let nextjs_routes = collect_nextjs_routes(root, &input, &symbols);
    symbols.extend(nextjs_routes);
    let angular_routes = angular::collect_angular_routes(root, &input, &symbols);
    symbols.extend(angular_routes);
    let relationships = collect_web_relationships(&symbols);

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some(language),
        symbols,
        relationships,
    })
}

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    let end_line = input.source.lines().count().max(1);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:module:{}", input.file_id.as_str(), input.path.display()),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("module")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line,
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: None,
    }
}

fn collect_web_symbols(node: Node<'_>, input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    if let Some((name, kind, visibility)) = web_symbol_name_kind_and_visibility(node, &input.source)
    {
        symbols.push(symbol_from_node(input, node, name, kind, visibility));
    }

    if let Some(import_name) = web_import_specifier(node, &input.source) {
        symbols.push(symbol_from_node(
            input,
            node,
            import_name,
            NodeKind::Package,
            None,
        ));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_web_symbols(child, input, symbols);
    }
}

fn collect_web_relationships(symbols: &[ExtractedSymbol]) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    collect_route_handler_relationships(symbols, &mut relationships);
    collect_component_relationships(symbols, &mut relationships);
    angular::collect_angular_relationships(symbols, &mut relationships);
    relationships
}

fn collect_component_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for component in symbols.iter().filter(|symbol| {
        component_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "framework",
        )
        .as_deref()
        .map(|framework| framework == "react" || framework == "nextjs" || framework == "angular")
        .unwrap_or(false)
    }) {
        let metadata = component.visibility.as_deref().unwrap_or_default();
        if let Some(props_type) = component_metadata_value(metadata, "props") {
            if let Some(target) = symbols.iter().find(|symbol| {
                matches!(symbol.kind, NodeKind::Interface | NodeKind::Variable)
                    && symbol.name == props_type
                    && symbol.id != component.id
            }) {
                relationships.push(index_edge(
                    &component.id,
                    &target.id,
                    EdgeKind::References,
                    EdgeProvenance::Ast,
                    8_000,
                ));
            }
        }

        if let Some(usages) = component_metadata_value(metadata, "usages") {
            for usage in usages.split(',').filter(|usage| !usage.is_empty()) {
                let usage_name = usage.rsplit('.').next().unwrap_or(usage).trim().to_string();
                if let Some(target) = symbols.iter().find(|symbol| {
                    symbol.name == usage_name
                        && symbol.id != component.id
                        && component_metadata_value(
                            symbol.visibility.as_deref().unwrap_or_default(),
                            "framework",
                        )
                        .as_deref()
                        .map(|framework| {
                            framework == "react" || framework == "nextjs" || framework == "angular"
                        })
                        .unwrap_or(false)
                }) {
                    relationships.push(index_edge(
                        &component.id,
                        &target.id,
                        EdgeKind::References,
                        EdgeProvenance::Ast,
                        8_000,
                    ));
                }
            }
        }
    }
}

fn collect_route_handler_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for route in symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
    {
        let Some(metadata) = route.visibility.as_deref() else {
            continue;
        };
        let handler = route_metadata_value(metadata, "handler")
            .or_else(|| route_metadata_value(metadata, "function"));
        let Some(handler) = handler else {
            continue;
        };
        let Some(target) = symbols.iter().find(|symbol| {
            matches!(symbol.kind, NodeKind::Function | NodeKind::Method)
                && symbol.name == handler
                && symbol.id != route.id
        }) else {
            continue;
        };
        relationships.push(index_edge(
            &route.id,
            &target.id,
            EdgeKind::References,
            EdgeProvenance::Ast,
            8_500,
        ));
    }
}

fn symbol_from_node(
    input: &ParseInput,
    node: Node<'_>,
    name: String,
    kind: NodeKind,
    visibility: Option<String>,
) -> ExtractedSymbol {
    let start = node.start_position();
    let end = node.end_position();
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{kind:?}:{name}:{}:{}",
                input.file_id.as_str(),
                node.start_byte(),
                node.end_byte()
            ),
        )),
        file_id: input.file_id.clone(),
        name,
        kind,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: one_based_row(start),
        start_column: start.column,
        end_line: one_based_row(end),
        end_column: end.column,
        visibility,
    }
}

fn web_symbol_name_kind_and_visibility(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let exported =
        has_parent_kind(node, "export_statement") || has_parent_kind(node, "export_clause");
    let visibility = exported.then(|| "export".to_string());
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            node.child_by_field_name("name").map(|name| {
                (
                    node_text(name, source).to_string(),
                    NodeKind::Function,
                    visibility,
                )
            })
        }
        "class_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Class,
                visibility,
            )
        }),
        "method_definition" | "method_signature" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Method,
                visibility,
            )
        }),
        "interface_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Interface,
                visibility,
            )
        }),
        "type_alias_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Variable,
                visibility,
            )
        }),
        "enum_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Enum,
                visibility,
            )
        }),
        "variable_declarator" => web_variable_symbol(node, source, visibility),
        "export_statement" => web_default_export_symbol(node, source),
        "assignment_expression" => web_module_exports_symbol(node, source),
        _ => None,
    }
}

fn web_variable_symbol(
    node: Node<'_>,
    source: &str,
    visibility: Option<String>,
) -> Option<(String, NodeKind, Option<String>)> {
    let name = node.child_by_field_name("name")?;
    let value = node.child_by_field_name("value");
    let value_kind = value.map(|value| value.kind());
    let exported = visibility.is_some();
    let should_index = exported
        || matches!(
            value_kind,
            Some(
                "arrow_function"
                    | "function"
                    | "function_expression"
                    | "class"
                    | "class_expression"
            )
        );
    if !should_index {
        return None;
    }

    let kind = if matches!(value_kind, Some("class" | "class_expression")) {
        NodeKind::Class
    } else if matches!(
        value_kind,
        Some("arrow_function" | "function" | "function_expression")
    ) {
        NodeKind::Function
    } else {
        NodeKind::Variable
    };
    Some((node_text(name, source).to_string(), kind, visibility))
}

fn web_default_export_symbol(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let text = node_text(node, source).trim_start();
    if !text.starts_with("export default") {
        return None;
    }
    if node
        .named_child(0)
        .map(|child| matches!(child.kind(), "function_declaration" | "class_declaration"))
        .unwrap_or(false)
    {
        return None;
    }
    Some((
        "default".to_string(),
        NodeKind::Variable,
        Some("export default".to_string()),
    ))
}

fn web_module_exports_symbol(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let left = node.child_by_field_name("left")?;
    let text = node_text(left, source).replace(' ', "");
    if text == "module.exports"
        || text.starts_with("module.exports.")
        || text.starts_with("exports.")
    {
        Some((
            node_text(left, source).trim().to_string(),
            NodeKind::Variable,
            Some("commonjs export".to_string()),
        ))
    } else {
        None
    }
}

fn annotate_react_components(root: Node<'_>, input: &ParseInput, symbols: &mut [ExtractedSymbol]) {
    let mut candidates = Vec::new();
    collect_react_component_candidates(root, input, &mut candidates);
    for (name, node, metadata) in candidates {
        if let Some(symbol) = symbols.iter_mut().find(|symbol| {
            symbol.name == name
                && symbol.start_byte <= node.start_byte()
                && symbol.end_byte >= node.end_byte()
                && matches!(symbol.kind, NodeKind::Function | NodeKind::Class)
        }) {
            symbol.visibility = merge_visibility(
                symbol.visibility.take(),
                encode_component_metadata(&metadata),
            );
        }
    }
}

fn collect_react_component_candidates<'a>(
    node: Node<'a>,
    input: &ParseInput,
    candidates: &mut Vec<(String, Node<'a>, ComponentMetadata)>,
) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) =
                    react_function_component_metadata(&name, node, input, "FunctionDeclaration")
                {
                    candidates.push((name, node, metadata));
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) = react_class_component_metadata(&name, node, input) {
                    candidates.push((name, node, metadata));
                }
            }
        }
        "variable_declarator" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) = react_variable_component_metadata(&name, node, input) {
                    candidates.push((name, node, metadata));
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_react_component_candidates(child, input, candidates);
    }
}

fn react_function_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
    source_kind: &str,
) -> Option<ComponentMetadata> {
    if !is_pascal_case(name) || !node_contains_jsx(node, &input.source) {
        return None;
    }
    let text = node_text(node, &input.source);
    let boundary = nextjs_component_boundary(&input.source, &input.path);
    Some(ComponentMetadata {
        framework: boundary
            .as_deref()
            .map(|_| "nextjs")
            .unwrap_or("react")
            .to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: boundary.unwrap_or_else(|| "function".to_string()),
        props_type_name: props_type_from_function_text(text),
        hooks: detect_hook_names(text),
        usages: detect_jsx_component_usages(text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_500,
        source_kind: source_kind.to_string(),
    })
}

fn react_class_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
) -> Option<ComponentMetadata> {
    let text = node_text(node, &input.source);
    if !is_pascal_case(name)
        || !(text.contains("React.Component") || text.contains("Component<"))
        || !node_contains_jsx(node, &input.source)
    {
        return None;
    }
    let boundary = nextjs_component_boundary(&input.source, &input.path);
    Some(ComponentMetadata {
        framework: boundary
            .as_deref()
            .map(|_| "nextjs")
            .unwrap_or("react")
            .to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: boundary.unwrap_or_else(|| "class".to_string()),
        props_type_name: type_between_after(text, "Component<"),
        hooks: Vec::new(),
        usages: detect_jsx_component_usages(text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_000,
        source_kind: "ClassComponent".to_string(),
    })
}

fn react_variable_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
) -> Option<ComponentMetadata> {
    if !is_pascal_case(name) {
        return None;
    }
    let text = node_text(node, &input.source);
    let value = node.child_by_field_name("value")?;
    let value_text = node_text(value, &input.source);
    let (component_kind, source_kind, confidence) = if value.kind() == "call_expression"
        && text.contains("memo")
        && node_contains_jsx(node, &input.source)
    {
        ("memo", "ReactMemo", 8_500)
    } else if value.kind() == "call_expression"
        && text.contains("forwardRef")
        && node_contains_jsx(node, &input.source)
    {
        ("forward_ref", "ReactForwardRef", 8_500)
    } else if matches!(
        value.kind(),
        "arrow_function" | "function" | "function_expression"
    ) && node_contains_jsx(value, &input.source)
    {
        ("arrow_function", "ArrowFunction", 9_500)
    } else {
        return None;
    };
    let boundary = nextjs_component_boundary(&input.source, &input.path);

    Some(ComponentMetadata {
        framework: boundary
            .as_deref()
            .map(|_| "nextjs")
            .unwrap_or("react")
            .to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: boundary.unwrap_or_else(|| component_kind.to_string()),
        props_type_name: props_type_from_variable_text(text),
        hooks: detect_hook_names(value_text),
        usages: detect_jsx_component_usages(value_text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence,
        source_kind: source_kind.to_string(),
    })
}

fn export_kind_for_node(node: Node<'_>, source: &str) -> Option<String> {
    if has_parent_kind(node, "export_statement") {
        let parent_text = ancestor_text(node, source, "export_statement").unwrap_or_default();
        if parent_text.trim_start().starts_with("export default") {
            Some("default".to_string())
        } else {
            Some("named".to_string())
        }
    } else {
        None
    }
}

fn nextjs_component_boundary(source: &str, path: &Path) -> Option<String> {
    if !path_has_segment(path, "app") {
        return None;
    }
    match top_of_file_directive(source).as_deref() {
        Some("use client") => Some("client_component".to_string()),
        Some("use server") => Some("server_component".to_string()),
        _ => Some("server_component".to_string()),
    }
}

fn top_of_file_directive(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let directive = trimmed.trim_end_matches(';');
        if directive == "\"use client\"" || directive == "'use client'" {
            return Some("use client".to_string());
        }
        if directive == "\"use server\"" || directive == "'use server'" {
            return Some("use server".to_string());
        }
        return None;
    }
    None
}

fn path_has_segment(path: &Path, segment: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy() == segment)
}

fn ancestor_text<'a>(node: Node<'a>, source: &'a str, kind: &str) -> Option<&'a str> {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == kind {
            return Some(node_text(current, source));
        }
        parent = current.parent();
    }
    None
}

fn node_contains_jsx(node: Node<'_>, source: &str) -> bool {
    let text = node_text(node, source);
    text.contains("</")
        || text.contains("/>")
        || text.contains("React.createElement")
        || text.contains("jsx(")
}

fn is_pascal_case(name: &str) -> bool {
    name.chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
}

fn props_type_from_function_text(text: &str) -> Option<String> {
    text.split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(params, _)| props_type_from_params(params))
}

fn props_type_from_variable_text(text: &str) -> Option<String> {
    type_between_after(text, "React.FC<")
        .or_else(|| type_between_after(text, "FC<"))
        .or_else(|| {
            text.split_once("=>")
                .and_then(|(before_arrow, _)| before_arrow.rsplit_once('('))
                .and_then(|(_, params)| props_type_from_params(params.trim_end_matches(')')))
        })
}

fn props_type_from_params(params: &str) -> Option<String> {
    params
        .split(':')
        .nth(1)
        .map(|value| {
            value
                .split(|character: char| {
                    character == ','
                        || character == '='
                        || character == ')'
                        || character.is_whitespace()
                })
                .find(|part| !part.is_empty())
                .unwrap_or_default()
                .trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn type_between_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find('>')?;
    let value = rest[..end]
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    (!value.is_empty()).then_some(value)
}

fn detect_hook_names(text: &str) -> Vec<String> {
    let mut hooks = Vec::new();
    for token in text.split(|character: char| !character.is_alphanumeric() && character != '_') {
        if is_hook_name(token) && !hooks.iter().any(|hook| hook == token) {
            hooks.push(token.to_string());
        }
    }
    hooks
}

fn is_hook_name(token: &str) -> bool {
    matches!(
        token,
        "useState"
            | "useEffect"
            | "useMemo"
            | "useCallback"
            | "useRef"
            | "useContext"
            | "useReducer"
    ) || token
        .strip_prefix("use")
        .and_then(|rest| rest.chars().next())
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
}

fn detect_jsx_component_usages(text: &str) -> Vec<String> {
    let mut usages = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(offset) = text[index..].find('<') {
        index += offset + 1;
        if index >= bytes.len() || bytes[index] == b'/' || bytes[index] == b'>' {
            continue;
        }
        let start = index;
        while index < bytes.len()
            && ((bytes[index] as char).is_ascii_alphanumeric()
                || bytes[index] == b'_'
                || bytes[index] == b'.')
        {
            index += 1;
        }
        let tag = &text[start..index];
        if tag
            .chars()
            .next()
            .map(|character| character.is_ascii_uppercase())
            .unwrap_or(false)
            && !usages.iter().any(|usage| usage == tag)
        {
            usages.push(tag.to_string());
        }
    }
    usages
}

fn encode_component_metadata(metadata: &ComponentMetadata) -> String {
    [
        ("component.framework", Some(metadata.framework.as_str())),
        ("component.export", metadata.export_kind.as_deref()),
        ("component.kind", Some(metadata.component_kind.as_str())),
        ("component.props", metadata.props_type_name.as_deref()),
        ("component.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", value.replace(';', "%3B"))))
    .chain([
        format!("component.hooks={}", metadata.hooks.join(",")),
        format!("component.usages={}", metadata.usages.join(",")),
        format!("component.line_start={}", metadata.line_start),
        format!("component.line_end={}", metadata.line_end),
        format!("component.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

pub(crate) fn component_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("component.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

fn merge_visibility(existing: Option<String>, metadata: String) -> Option<String> {
    match existing {
        Some(existing) if !existing.is_empty() => Some(format!("{existing};{metadata}")),
        _ => Some(metadata),
    }
}

fn web_import_specifier(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "import_statement" => node
            .child_by_field_name("source")
            .and_then(|source_node| string_literal_value(source_node, source)),
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            if node_text(function, source) != "require" {
                return None;
            }
            let arguments = node.child_by_field_name("arguments")?;
            first_string_child(arguments, source)
        }
        _ => None,
    }
}

pub fn resolve_web_import_path(importer_path: &Path, specifier: &str) -> Option<PathBuf> {
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return None;
    }
    let base = importer_path.parent()?.join(specifier);
    web_import_candidates(&base)
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn web_import_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(base.to_path_buf());
    } else {
        for extension in ["js", "jsx", "ts", "tsx"] {
            candidates.push(base.with_extension(extension));
        }
        for extension in ["js", "jsx", "ts", "tsx"] {
            candidates.push(base.join(format!("index.{extension}")));
        }
    }
    candidates
}

fn collect_node_rest_routes(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut routes = Vec::new();
    collect_call_routes(root, input, &mut routes);
    collect_nest_routes(root, input, symbols, &mut routes);
    routes
}

fn collect_nextjs_routes(
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

fn collect_call_routes(node: Node<'_>, input: &ParseInput, routes: &mut Vec<ExtractedSymbol>) {
    if node.kind() == "call_expression" {
        if let Some(metadata) = express_or_fastify_route_metadata(node, input) {
            routes.push(route_symbol(input, node, &metadata));
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_routes(child, input, routes);
    }
}

fn express_or_fastify_route_metadata(node: Node<'_>, input: &ParseInput) -> Option<RouteMetadata> {
    let function = node.child_by_field_name("function")?;
    let function_text = compact_member_text(node_text(function, &input.source));
    if function_text.ends_with(".route") && function_text.contains("fastify") {
        return fastify_route_object_metadata(node, input);
    }
    let route_like_receiver = function_text.starts_with("app.")
        || function_text.starts_with("router.")
        || function_text.starts_with("fastify.")
        || function_text.contains(".route(");
    if !route_like_receiver {
        return None;
    }

    let method = route_method_from_function_text(&function_text)?;
    let (framework, source_kind, path) =
        if let Some(route_path) = route_path_from_chained_route_call(&function_text) {
            ("express", "ExpressRouterCall", route_path)
        } else {
            let arguments = node.child_by_field_name("arguments")?;
            let path = first_string_child(arguments, &input.source)?;
            let framework = if function_text.contains("fastify") {
                "fastify"
            } else {
                "express"
            };
            let source_kind = if framework == "fastify" {
                "FastifyShorthandCall"
            } else if function_text.contains("router.") {
                "ExpressRouterCall"
            } else {
                "ExpressCall"
            };
            (framework, source_kind, path)
        };
    let handler_name = node
        .child_by_field_name("arguments")
        .and_then(|arguments| nth_argument_name(arguments, &input.source, 1));
    let confidence = if source_kind == "ExpressRouterCall" {
        9_000
    } else {
        9_500
    };
    Some(RouteMetadata {
        framework: framework.to_string(),
        route_kind: "api".to_string(),
        method,
        path: normalize_route_path("", &path),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: handler_name.clone(),
        class_name: None,
        function_name: handler_name,
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence,
        source_kind: source_kind.to_string(),
    })
}

fn fastify_route_object_metadata(node: Node<'_>, input: &ParseInput) -> Option<RouteMetadata> {
    let arguments = node.child_by_field_name("arguments")?;
    let object = first_child_kind(arguments, "object")?;
    let object_text = node_text(object, &input.source);
    let method = object_property_string(object_text, "method")
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let path = object_property_string(object_text, "url")
        .or_else(|| object_property_string(object_text, "path"))?;
    let handler_name = object_property_identifier(object_text, "handler");
    Some(RouteMetadata {
        framework: "fastify".to_string(),
        route_kind: "api".to_string(),
        method,
        path: normalize_route_path("", &path),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: handler_name.clone(),
        class_name: None,
        function_name: handler_name,
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_000,
        source_kind: "FastifyRouteCall".to_string(),
    })
}

fn collect_nest_routes(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    routes: &mut Vec<ExtractedSymbol>,
) {
    if node.kind() == "class_declaration" {
        collect_nest_class_routes(node, input, symbols, routes);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nest_routes(child, input, symbols, routes);
    }
}

fn collect_nest_class_routes(
    class_node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    routes: &mut Vec<ExtractedSymbol>,
) {
    let class_text = node_text(class_node, &input.source);
    let leading_text = leading_decorator_text(class_node, &input.source);
    let Some(controller_path) = decorator_argument(&leading_text, "Controller")
        .or_else(|| decorator_argument(class_text, "Controller"))
    else {
        return;
    };
    let class_name = class_node
        .child_by_field_name("name")
        .map(|name| node_text(name, &input.source).to_string());
    let mut cursor = class_node.walk();
    for child in class_node.named_children(&mut cursor) {
        collect_nest_method_routes(
            child,
            input,
            symbols,
            &controller_path,
            class_name.as_deref(),
            routes,
        );
    }
}

fn collect_nest_method_routes(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    controller_path: &str,
    class_name: Option<&str>,
    routes: &mut Vec<ExtractedSymbol>,
) {
    if matches!(node.kind(), "method_definition" | "method_signature") {
        let text = format!(
            "{}\n{}",
            leading_decorator_text(node, &input.source),
            node_text(node, &input.source)
        );
        if let Some((method, method_path)) = nest_method_decorator(&text) {
            let function_name = node
                .child_by_field_name("name")
                .map(|name| node_text(name, &input.source).to_string());
            let symbol_id = function_name.as_ref().and_then(|name| {
                symbols
                    .iter()
                    .find(|symbol| {
                        symbol.kind == NodeKind::Method
                            && &symbol.name == name
                            && symbol.start_byte <= node.start_byte()
                            && symbol.end_byte >= node.end_byte()
                    })
                    .map(|symbol| symbol.id.clone())
            });
            let metadata = RouteMetadata {
                framework: "nestjs".to_string(),
                route_kind: "api".to_string(),
                method,
                path: normalize_route_path(controller_path, &method_path),
                file_path: input.path.to_string_lossy().replace('\\', "/"),
                symbol_id,
                handler_name: function_name.clone(),
                class_name: class_name.map(str::to_string),
                function_name,
                line_start: one_based_row(node.start_position()),
                line_end: one_based_row(node.end_position()),
                confidence: 9_500,
                source_kind: "NestMethodDecorator".to_string(),
            };
            routes.push(route_symbol(input, node, &metadata));
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nest_method_routes(child, input, symbols, controller_path, class_name, routes);
    }
}

fn route_symbol(input: &ParseInput, node: Node<'_>, metadata: &RouteMetadata) -> ExtractedSymbol {
    let mut symbol = symbol_from_node(
        input,
        node,
        format!("{} {}", metadata.method, metadata.path),
        NodeKind::Route,
        Some(encode_route_metadata(metadata)),
    );
    symbol.id = SymbolId::new(stable_id(
        "symbol",
        &format!(
            "{}:route:{}:{}:{}:{}",
            input.file_id.as_str(),
            metadata.framework,
            metadata.method,
            metadata.path,
            node.start_byte()
        ),
    ));
    symbol
}

pub fn detect_package_json_technologies(source: &str) -> ContractResult<Vec<DetectedTechnology>> {
    let value = serde_json::from_str::<serde_json::Value>(source)
        .map_err(|error| ContractError::new(format!("invalid package.json: {error}")))?;
    let mut technologies = Vec::new();
    let dependencies = ["dependencies", "devDependencies", "peerDependencies"];
    for section in dependencies {
        let Some(object) = value.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for package_name in object.keys() {
            if let Some(technology) = package_technology(package_name, section) {
                if !technologies
                    .iter()
                    .any(|existing: &DetectedTechnology| existing.id == technology.id)
                {
                    technologies.push(technology);
                }
            }
        }
    }
    Ok(technologies)
}

fn package_technology(package_name: &str, section: &str) -> Option<DetectedTechnology> {
    let (id, name, kind, support_level, capabilities) = match package_name {
        "express" => (
            "express",
            "Express",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "@nestjs/core" | "@nestjs/common" => (
            "nestjs",
            "NestJS",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "fastify" => (
            "fastify",
            "Fastify",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "typescript" | "ts-node" => (
            "typescript",
            "TypeScript",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![TechnologyCapability::DetectPackage],
        ),
        "react" | "react-dom" | "@types/react" => (
            "react",
            "React",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractComponents,
            ],
        ),
        "next" => (
            "nextjs",
            "Next.js",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
                TechnologyCapability::ExtractComponents,
            ],
        ),
        "@angular/core"
        | "@angular/common"
        | "@angular/router"
        | "@angular/forms"
        | "@angular/platform-browser"
        | "@angular/cli" => (
            "angular",
            "Angular",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
                TechnologyCapability::ExtractComponents,
            ],
        ),
        "vite" => (
            package_name,
            package_name,
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::DetectOnly,
            vec![TechnologyCapability::DetectPackage],
        ),
        name if name.starts_with("@fastify/") => (
            "fastify",
            "Fastify",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![TechnologyCapability::DetectPackage],
        ),
        _ => return None,
    };
    Some(DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        support_level,
        capabilities,
        source: format!("package.json:{section}:{package_name}"),
    })
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

pub fn detect_angular_config_path(path: &Path) -> Option<DetectedTechnology> {
    let file_name = path.file_name()?.to_str()?;
    if !matches!(file_name, "angular.json" | "tsconfig.app.json") {
        return None;
    }
    Some(DetectedTechnology {
        id: "angular".to_string(),
        name: "Angular".to_string(),
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

fn route_method_from_function_text(function_text: &str) -> Option<String> {
    for (suffix, method) in [
        (".get", "GET"),
        (".post", "POST"),
        (".put", "PUT"),
        (".patch", "PATCH"),
        (".delete", "DELETE"),
        (".options", "OPTIONS"),
        (".head", "HEAD"),
        (".all", "ALL"),
        (".use", "ALL"),
    ] {
        if function_text.ends_with(suffix) {
            return Some(method.to_string());
        }
    }
    None
}

fn route_path_from_chained_route_call(function_text: &str) -> Option<String> {
    let route_start = function_text.find(".route(")?;
    let after_route = &function_text[route_start + ".route(".len()..];
    let quote = after_route
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_route.split_once(quote)?.1;
    let path = after_quote.split_once(quote)?.0;
    Some(path.to_string())
}

fn nth_argument_name(arguments: Node<'_>, source: &str, index: usize) -> Option<String> {
    let mut cursor = arguments.walk();
    let value = arguments
        .named_children(&mut cursor)
        .filter(|child| child.kind() != "comment")
        .nth(index)
        .and_then(|node| match node.kind() {
            "identifier" => Some(node_text(node, source).to_string()),
            "member_expression" => Some(node_text(node, source).to_string()),
            "arrow_function" | "function" | "function_expression" => None,
            _ => Some(node_text(node, source).trim().to_string()).filter(|value| !value.is_empty()),
        });
    value
}

fn first_child_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    value
}

fn object_property_string(object_text: &str, key: &str) -> Option<String> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    let after_colon = after_key[colon_position + 1..].trim_start();
    let quote = after_colon
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_colon.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

fn object_property_identifier(object_text: &str, key: &str) -> Option<String> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    let after_colon = after_key[colon_position + 1..].trim_start();
    let value = after_colon
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '.'
        })
        .next()
        .unwrap_or_default();
    (!value.is_empty()).then(|| value.to_string())
}

fn decorator_argument(text: &str, decorator_name: &str) -> Option<String> {
    let needle = format!("@{decorator_name}");
    let position = text.find(&needle)?;
    let after = &text[position + needle.len()..];
    let open = after.find('(')?;
    let after_open = after[open + 1..].trim_start();
    if after_open.starts_with(')') {
        return Some(String::new());
    }
    let quote = after_open
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_open.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

fn leading_decorator_text(node: Node<'_>, source: &str) -> String {
    let mut parts = Vec::new();
    let mut sibling = node.prev_named_sibling();
    while let Some(value) = sibling {
        let text = node_text(value, source).trim();
        if !text.starts_with('@') {
            break;
        }
        parts.push(text.to_string());
        sibling = value.prev_named_sibling();
    }
    if parts.is_empty() {
        if let Some(parent) = node.parent() {
            let mut parent_sibling = parent.prev_named_sibling();
            while let Some(value) = parent_sibling {
                let text = node_text(value, source).trim();
                if !text.starts_with('@') {
                    break;
                }
                parts.push(text.to_string());
                parent_sibling = value.prev_named_sibling();
            }
        }
    }
    parts.reverse();
    parts.join("\n")
}

fn nest_method_decorator(text: &str) -> Option<(String, String)> {
    for (decorator, method) in [
        ("Get", "GET"),
        ("Post", "POST"),
        ("Put", "PUT"),
        ("Patch", "PATCH"),
        ("Delete", "DELETE"),
        ("Options", "OPTIONS"),
        ("Head", "HEAD"),
        ("All", "ALL"),
    ] {
        let needle = format!("@{decorator}");
        if text.contains(&needle) {
            return Some((
                method.to_string(),
                decorator_argument(text, decorator).unwrap_or_default(),
            ));
        }
    }
    None
}

fn normalize_route_path(base: &str, path: &str) -> String {
    let clean_base = base.trim_matches('/');
    let clean_path = path.trim_matches('/');
    match (clean_base.is_empty(), clean_path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{clean_path}"),
        (false, true) => format!("/{clean_base}"),
        (false, false) => format!("/{clean_base}/{clean_path}"),
    }
}

fn encode_route_metadata(metadata: &RouteMetadata) -> String {
    [
        ("route.framework", Some(metadata.framework.as_str())),
        ("route.kind", Some(metadata.route_kind.as_str())),
        ("route.method", Some(metadata.method.as_str())),
        ("route.path", Some(metadata.path.as_str())),
        ("route.file", Some(metadata.file_path.as_str())),
        ("route.handler", metadata.handler_name.as_deref()),
        ("route.class", metadata.class_name.as_deref()),
        ("route.function", metadata.function_name.as_deref()),
        ("route.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", value.replace(';', "%3B"))))
    .chain([
        format!("route.line_start={}", metadata.line_start),
        format!("route.line_end={}", metadata.line_end),
        format!("route.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

pub(crate) fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("route.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

fn compact_member_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn first_string_child(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find_map(|child| string_literal_value(child, source));
    value
}

fn string_literal_value(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(node.kind(), "string" | "string_fragment") {
        return None;
    }
    let text = node_text(node, source).trim();
    Some(
        text.trim_matches('"')
            .trim_matches('\'')
            .trim_matches('`')
            .to_string(),
    )
}
