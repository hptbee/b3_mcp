use super::*;

#[derive(Debug, Clone)]
struct GoDecl {
    name: String,
    kind: NodeKind,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
    receiver: Option<String>,
    metadata: String,
}

#[derive(Debug, Clone)]
struct GoRouteHint {
    framework: String,
    method: String,
    path: String,
    handler: Option<String>,
    line: usize,
    start_byte: usize,
    end_byte: usize,
    source_kind: String,
    confidence: u16,
}

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    match language_from_path(&input.path).as_deref() {
        Some("go") => parse_go_file(input),
        Some("gomod") => parse_go_mod(input),
        _ => NoopTreeSitterParser.parse(input),
    }
}

fn parse_go_file(input: ParseInput) -> ContractResult<ParsedFile> {
    let clean = strip_go_comments_preserve_lines(&input.source);
    let mut symbols = vec![module_symbol(&input)];
    let package_name = collect_package_symbol(&input, &clean, &mut symbols);
    collect_import_symbols(&input, &clean, &mut symbols);
    let declarations = collect_declarations(&input, &clean);
    for declaration in &declarations {
        symbols.push(declaration_symbol(
            &input,
            declaration,
            package_name.as_deref(),
        ));
    }
    collect_route_hints(&input, &clean, &mut symbols);
    let relationships = collect_go_relationships(&input, &clean, &symbols);

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("go".to_string()),
        symbols,
        relationships,
    })
}

fn parse_go_mod(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input)];
    let mut metadata = "go.mod=true".to_string();
    if let Some(module_path) = go_mod_module_path(&input.source) {
        append_metadata(&mut metadata, "go.module", &module_path);
        symbols.push(simple_symbol(
            &input,
            &module_path,
            NodeKind::Package,
            1,
            Some(metadata.clone()),
        ));
    }
    for technology in detect_go_mod_technologies(&input.source)? {
        symbols.push(simple_symbol(
            &input,
            &technology.name,
            NodeKind::Package,
            1,
            Some(format!(
                "go.technology={};go.support={:?};go.source={}",
                technology.id, technology.support_level, technology.source
            )),
        ));
    }
    for requirement in go_mod_requires(&input.source) {
        symbols.push(simple_symbol(
            &input,
            &requirement,
            NodeKind::Package,
            1,
            Some(format!("go.require=true;go.import_path={requirement}")),
        ));
    }
    for replacement in go_mod_replaces(&input.source) {
        symbols.push(simple_symbol(
            &input,
            &replacement,
            NodeKind::Package,
            1,
            Some(format!("go.replace=true;go.import_path={replacement}")),
        ));
    }

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("gomod".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

pub(crate) fn detect_go_mod_technologies(source: &str) -> ContractResult<Vec<DetectedTechnology>> {
    let mut detected = Vec::new();
    if go_mod_module_path(source).is_some() {
        detected.push(DetectedTechnology {
            id: "go".to_string(),
            name: "Go".to_string(),
            kind: TechnologyKind::Language,
            support_level: TechnologySupportLevel::Basic,
            capabilities: vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractSymbols,
                TechnologyCapability::ExtractRoutes,
            ],
            source: "go.mod".to_string(),
        });
    }
    Ok(detected)
}

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:go-module:{}",
                input.file_id.as_str(),
                input.path.display()
            ),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("go-file")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line: input.source.lines().count().max(1),
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some("go.file=true".to_string()),
    }
}

fn collect_package_symbol(
    input: &ParseInput,
    clean: &str,
    symbols: &mut Vec<ExtractedSymbol>,
) -> Option<String> {
    for (index, line) in clean.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("package ") {
            let name = rest
                .split(|ch: char| !is_go_identifier_char(ch))
                .next()
                .unwrap_or_default();
            if !name.is_empty() {
                symbols.push(simple_symbol(
                    input,
                    name,
                    NodeKind::Namespace,
                    index + 1,
                    Some(format!("go.package=true;go.name={name}")),
                ));
                return Some(name.to_string());
            }
        }
    }
    None
}

fn collect_import_symbols(input: &ParseInput, clean: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let lines: Vec<&str> = clean.lines().collect();
    let mut index = 0usize;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed == "import (" {
            index += 1;
            while index < lines.len() && !lines[index].trim().starts_with(')') {
                if let Some(import) = parse_import_line(lines[index].trim()) {
                    symbols.push(import_symbol(input, &import, index + 1));
                }
                index += 1;
            }
        } else if let Some(rest) = trimmed.strip_prefix("import ") {
            if let Some(import) = parse_import_line(rest.trim()) {
                symbols.push(import_symbol(input, &import, index + 1));
            }
        }
        index += 1;
    }
}

fn parse_import_line(line: &str) -> Option<(Option<String>, String)> {
    let quote = line.find('"')?;
    let before = line[..quote].trim();
    let rest = &line[quote + 1..];
    let end = rest.find('"')?;
    let alias = (!before.is_empty()).then(|| before.to_string());
    Some((alias, rest[..end].to_string()))
}

fn import_symbol(
    input: &ParseInput,
    import: &(Option<String>, String),
    line: usize,
) -> ExtractedSymbol {
    let (alias, path) = import;
    let mut metadata = format!(
        "go.import=true;go.import_path={};go.stdlib={}",
        escape_metadata(path),
        bool_str(is_go_stdlib_import(path))
    );
    if let Some(alias) = alias {
        append_metadata(&mut metadata, "go.alias", alias);
    }
    simple_symbol(input, path, NodeKind::Package, line, Some(metadata))
}

fn collect_declarations(input: &ParseInput, clean: &str) -> Vec<GoDecl> {
    let lines: Vec<&str> = clean.lines().collect();
    let offsets = line_offsets(clean);
    let mut declarations = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if let Some((name, receiver)) = parse_go_function(trimmed) {
            let start_byte = offsets[index];
            let end_byte =
                find_block_end(clean, start_byte).unwrap_or(start_byte + lines[index].len());
            let end_line = byte_to_line(&offsets, end_byte);
            let mut metadata = "go.function=true".to_string();
            let kind = if let Some(receiver) = &receiver {
                append_metadata(&mut metadata, "go.receiver", &receiver);
                NodeKind::Method
            } else {
                NodeKind::Function
            };
            append_metadata(&mut metadata, "go.exported", bool_str(is_exported(&name)));
            declarations.push(GoDecl {
                name,
                kind,
                start_byte,
                end_byte,
                start_line: index + 1,
                end_line,
                receiver,
                metadata,
            });
        } else if let Some((name, kind, detail)) = parse_go_type(trimmed) {
            declarations.push(GoDecl {
                name: name.clone(),
                kind,
                start_byte: offsets[index],
                end_byte: offsets[index] + lines[index].len(),
                start_line: index + 1,
                end_line: index + 1,
                receiver: None,
                metadata: format!(
                    "go.type=true;go.type_kind={detail};go.exported={}",
                    bool_str(is_exported(&name))
                ),
            });
        } else if let Some(name) = parse_go_value_decl(trimmed, "const") {
            declarations.push(value_decl(input, &name, "const", index + 1, offsets[index]));
        } else if let Some(name) = parse_go_value_decl(trimmed, "var") {
            declarations.push(value_decl(input, &name, "var", index + 1, offsets[index]));
        }
        index += 1;
    }
    declarations
}

fn value_decl(
    input: &ParseInput,
    name: &str,
    decl_kind: &str,
    line: usize,
    start_byte: usize,
) -> GoDecl {
    let _ = input;
    GoDecl {
        name: name.to_string(),
        kind: NodeKind::Variable,
        start_byte,
        end_byte: start_byte,
        start_line: line,
        end_line: line,
        receiver: None,
        metadata: format!(
            "go.value=true;go.value_kind={decl_kind};go.exported={}",
            bool_str(is_exported(name))
        ),
    }
}

fn parse_go_function(line: &str) -> Option<(String, Option<String>)> {
    let rest = line.strip_prefix("func ")?;
    if let Some(receiver_rest) = rest.strip_prefix('(') {
        let end_receiver = receiver_rest.find(')')?;
        let receiver = receiver_type(&receiver_rest[..end_receiver]);
        let after = receiver_rest[end_receiver + 1..].trim_start();
        let name = after.split('(').next()?.trim();
        if is_go_identifier(name) {
            return Some((name.to_string(), receiver));
        }
    }
    let name = rest.split('(').next()?.trim();
    is_go_identifier(name).then(|| (name.to_string(), None))
}

fn receiver_type(receiver: &str) -> Option<String> {
    let value = receiver
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_start_matches('*')
        .trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_go_type(line: &str) -> Option<(String, NodeKind, String)> {
    let rest = line.strip_prefix("type ")?;
    let name = rest.split_whitespace().next().unwrap_or_default().trim();
    if !is_go_identifier(name) {
        return None;
    }
    let after = rest[name.len()..].trim_start();
    if after.starts_with("struct") {
        Some((name.to_string(), NodeKind::Struct, "struct".to_string()))
    } else if after.starts_with("interface") {
        Some((
            name.to_string(),
            NodeKind::Interface,
            "interface".to_string(),
        ))
    } else if after.starts_with('=') {
        Some((name.to_string(), NodeKind::Variable, "alias".to_string()))
    } else {
        Some((name.to_string(), NodeKind::Variable, "type".to_string()))
    }
}

fn parse_go_value_decl(line: &str, keyword: &str) -> Option<String> {
    let rest = line.strip_prefix(keyword)?.trim_start();
    if rest.starts_with('(') {
        return None;
    }
    let name = rest
        .split(|ch: char| ch.is_whitespace() || ch == '=' || ch == ',')
        .next()
        .unwrap_or_default();
    is_go_identifier(name).then(|| name.to_string())
}

fn declaration_symbol(
    input: &ParseInput,
    declaration: &GoDecl,
    package_name: Option<&str>,
) -> ExtractedSymbol {
    let mut metadata = declaration.metadata.clone();
    if let Some(package_name) = package_name {
        append_metadata(&mut metadata, "go.package", package_name);
    }
    if let Some(receiver) = &declaration.receiver {
        append_metadata(&mut metadata, "go.receiver", receiver);
    }
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:go:{:?}:{}:{}",
                input.file_id.as_str(),
                declaration.kind,
                declaration.name,
                declaration.start_byte
            ),
        )),
        file_id: input.file_id.clone(),
        name: declaration.name.clone(),
        kind: declaration.kind,
        start_byte: declaration.start_byte,
        end_byte: declaration.end_byte,
        start_line: declaration.start_line,
        start_column: 0,
        end_line: declaration.end_line,
        end_column: 0,
        visibility: Some(metadata),
    }
}

fn collect_route_hints(input: &ParseInput, clean: &str, symbols: &mut Vec<ExtractedSymbol>) {
    for hint in go_route_hints(clean) {
        let metadata = RouteMetadata {
            framework: hint.framework,
            route_kind: "http".to_string(),
            method: hint.method,
            path: hint.path,
            file_path: input.path.to_string_lossy().replace('\\', "/"),
            symbol_id: None,
            handler_name: hint.handler,
            class_name: None,
            function_name: None,
            line_start: hint.line,
            line_end: hint.line,
            confidence: hint.confidence,
            source_kind: hint.source_kind,
        };
        symbols.push(ExtractedSymbol {
            id: SymbolId::new(stable_id(
                "symbol",
                &format!(
                    "{}:go-route:{}:{}:{}",
                    input.file_id.as_str(),
                    metadata.framework,
                    metadata.method,
                    hint.start_byte
                ),
            )),
            file_id: input.file_id.clone(),
            name: format!("{} {}", metadata.method, metadata.path),
            kind: NodeKind::Route,
            start_byte: hint.start_byte,
            end_byte: hint.end_byte,
            start_line: hint.line,
            start_column: 0,
            end_line: hint.line,
            end_column: 0,
            visibility: Some(encode_route_metadata(&metadata)),
        });
    }
}

fn go_route_hints(clean: &str) -> Vec<GoRouteHint> {
    let mut hints = Vec::new();
    let offsets = line_offsets(clean);
    let router_vars = router_framework_variables(clean);
    for (index, line) in clean.lines().enumerate() {
        let line_start = offsets[index];
        if let Some((callee, method, framework, source_kind, confidence)) =
            route_callee(line, &router_vars)
        {
            let Some(callee_offset) = line.find(callee.as_str()) else {
                continue;
            };
            if line
                .find('"')
                .is_some_and(|first_quote| first_quote < callee_offset)
            {
                continue;
            }
            let Some(args_start) = line[callee_offset..]
                .find('(')
                .map(|pos| callee_offset + pos)
            else {
                continue;
            };
            let args = &line[args_start + 1..];
            let Some(path) = first_string_literal(args) else {
                continue;
            };
            let handler = route_handler_arg(args);
            hints.push(GoRouteHint {
                framework,
                method,
                path,
                handler,
                line: index + 1,
                start_byte: line_start + callee_offset,
                end_byte: line_start + line.len(),
                source_kind,
                confidence,
            });
        }
    }
    hints
}

fn route_callee(
    line: &str,
    router_vars: &[(String, String)],
) -> Option<(String, String, String, String, u16)> {
    for (callee, method) in [("http.HandleFunc", "GET"), ("http.Handle", "GET")] {
        if line.contains(callee) {
            return Some((
                callee.to_string(),
                method.to_string(),
                "go_net_http".to_string(),
                if callee == "http.HandleFunc" {
                    "GoNetHttpHandleFunc"
                } else {
                    "GoNetHttpHandle"
                }
                .to_string(),
                9_000,
            ));
        }
    }

    for (var, framework) in router_vars {
        for (method_name, method) in [
            ("GET", "GET"),
            ("POST", "POST"),
            ("PUT", "PUT"),
            ("PATCH", "PATCH"),
            ("DELETE", "DELETE"),
            ("Get", "GET"),
            ("Post", "POST"),
            ("Put", "PUT"),
            ("Patch", "PATCH"),
            ("Delete", "DELETE"),
        ] {
            let callee = format!("{var}.{method_name}");
            if !line.contains(&callee) {
                continue;
            }
            let source = match framework.as_str() {
                "gin" => "GinRouteHint",
                "echo" => "EchoRouteHint",
                "fiber" => "FiberRouteHint",
                "chi" => "ChiRouteHint",
                _ => "GoRouterRouteHint",
            };
            return Some((
                callee,
                method.to_string(),
                framework.clone(),
                source.to_string(),
                7_500,
            ));
        }
    }
    None
}

fn router_framework_variables(clean: &str) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    for line in clean.lines() {
        let trimmed = line.trim();
        let Some((left, right)) = trimmed.split_once(":=") else {
            continue;
        };
        let var = left.trim();
        if !is_go_identifier(var) {
            continue;
        }
        let framework = if right.contains("gin.Default(") || right.contains("gin.New(") {
            Some("gin")
        } else if right.contains("echo.New(") {
            Some("echo")
        } else if right.contains("fiber.New(") {
            Some("fiber")
        } else if right.contains("chi.NewRouter(") || right.contains("chi.NewMux(") {
            Some("chi")
        } else {
            None
        };
        if let Some(framework) = framework {
            vars.push((var.to_string(), framework.to_string()));
        }
    }
    vars
}

fn route_handler_arg(args: &str) -> Option<String> {
    let after_path = args
        .find('"')
        .and_then(|start| args[start + 1..].find('"').map(|end| start + 1 + end + 1))?;
    let handler = args[after_path..]
        .trim_start()
        .strip_prefix(',')?
        .trim_start()
        .split([',', ')'])
        .next()
        .unwrap_or_default()
        .trim();
    is_go_selector_or_identifier(handler).then(|| handler.to_string())
}

fn collect_go_relationships(
    input: &ParseInput,
    clean: &str,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    collect_local_call_relationships(input, clean, symbols, &mut relationships);
    collect_route_handler_relationships(symbols, &mut relationships);
    relationships
}

fn collect_local_call_relationships(
    _input: &ParseInput,
    clean: &str,
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let local_callables: Vec<&ExtractedSymbol> = symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, NodeKind::Function | NodeKind::Method))
        .collect();
    for caller in &local_callables {
        let body = clean
            .get(caller.start_byte..caller.end_byte.min(clean.len()))
            .unwrap_or_default();
        for callee in &local_callables {
            if caller.id == callee.id {
                continue;
            }
            let needle = format!("{}(", callee.name);
            if body.contains(&needle) {
                relationships.push(index_edge(
                    &caller.id,
                    &callee.id,
                    EdgeKind::Calls,
                    EdgeProvenance::TextHeuristic,
                    7_500,
                ));
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
        let Some(handler) = route_metadata_value(metadata, "handler")
            .and_then(|value| value.rsplit('.').next().map(str::to_string))
        else {
            continue;
        };
        if let Some(target) = symbols.iter().find(|symbol| {
            matches!(symbol.kind, NodeKind::Function | NodeKind::Method)
                && symbol.name == handler
                && symbol.id != route.id
        }) {
            relationships.push(index_edge(
                &route.id,
                &target.id,
                EdgeKind::References,
                EdgeProvenance::TextHeuristic,
                8_000,
            ));
        }
    }
}

fn simple_symbol(
    input: &ParseInput,
    name: &str,
    kind: NodeKind,
    line: usize,
    visibility: Option<String>,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:go:{kind:?}:{name}:{line}", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name: name.to_string(),
        kind,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: line,
        end_column: 0,
        visibility,
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
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("route.line_start={}", metadata.line_start),
        format!("route.line_end={}", metadata.line_end),
        format!("route.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("route.{key}="))
            .map(unescape_metadata)
    })
}

#[cfg(test)]
pub(crate) fn go_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("go.{key}="))
            .map(unescape_metadata)
    })
}

fn go_mod_module_path(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("module ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn go_mod_requires(source: &str) -> Vec<String> {
    collect_go_mod_directives(source, "require")
}

fn go_mod_replaces(source: &str) -> Vec<String> {
    collect_go_mod_directives(source, "replace")
}

fn collect_go_mod_directives(source: &str, keyword: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut in_block = false;
    for line in source.lines() {
        let trimmed = line.split("//").next().unwrap_or_default().trim();
        if trimmed == format!("{keyword} (") {
            in_block = true;
            continue;
        }
        if in_block && trimmed.starts_with(')') {
            in_block = false;
            continue;
        }
        let candidate = if in_block {
            trimmed
        } else {
            trimmed.strip_prefix(keyword).unwrap_or_default().trim()
        };
        if candidate.is_empty() {
            continue;
        }
        let module = candidate
            .split("=>")
            .next()
            .unwrap_or(candidate)
            .split_whitespace()
            .next()
            .unwrap_or_default();
        if !module.is_empty() && module.contains('/') {
            values.push(module.to_string());
        }
    }
    values
}

fn strip_go_comments_preserve_lines(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_line = false;
    let mut in_block = false;
    let mut in_string = false;
    let mut in_raw = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_line {
            if ch == '\n' {
                in_line = false;
                output.push('\n');
            } else {
                output.push(' ');
            }
            continue;
        }
        if in_block {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
                output.push(' ');
                output.push(' ');
            } else if ch == '\n' {
                output.push('\n');
            } else {
                output.push(' ');
            }
            continue;
        }
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if in_raw {
            output.push(ch);
            if ch == '`' {
                in_raw = false;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line = true;
            output.push(' ');
            output.push(' ');
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block = true;
            output.push(' ');
            output.push(' ');
        } else {
            if ch == '"' {
                in_string = true;
            } else if ch == '`' {
                in_raw = true;
            }
            output.push(ch);
        }
    }
    output
}

fn first_string_literal(value: &str) -> Option<String> {
    let start = value.find('"')?;
    let rest = &value[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn is_go_stdlib_import(path: &str) -> bool {
    !path.contains('.')
}

fn is_exported(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn is_go_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(is_go_identifier_char)
}

fn is_go_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_go_selector_or_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|part| is_go_identifier(part.trim_start_matches('*')))
}

fn bool_str(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn append_metadata(metadata: &mut String, key: &str, value: &str) {
    if !metadata.is_empty() {
        metadata.push(';');
    }
    metadata.push_str(key);
    metadata.push('=');
    metadata.push_str(&escape_metadata(value));
}

fn escape_metadata(value: &str) -> String {
    value.replace(';', "%3B")
}

fn unescape_metadata(value: &str) -> String {
    value.replace("%3B", ";")
}

fn line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(index + 1);
        }
    }
    offsets
}

fn byte_to_line(offsets: &[usize], byte: usize) -> usize {
    offsets
        .iter()
        .enumerate()
        .take_while(|(_, offset)| **offset <= byte)
        .last()
        .map(|(index, _)| index + 1)
        .unwrap_or(1)
}

fn find_block_end(source: &str, start_byte: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut seen_open = false;
    for (offset, ch) in source.get(start_byte..)?.char_indices() {
        match ch {
            '{' => {
                seen_open = true;
                depth += 1;
            }
            '}' if seen_open => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(start_byte + offset + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}
