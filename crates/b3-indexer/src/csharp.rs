use super::*;

#[derive(Debug, Clone)]
struct AttributeInfo {
    name: String,
    argument: Option<String>,
}

#[derive(Debug, Clone)]
struct ClassInfo {
    name: String,
    base_text: Option<String>,
    attrs: Vec<AttributeInfo>,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
}

#[derive(Debug, Clone)]
struct MethodInfo {
    name: String,
    attrs: Vec<AttributeInfo>,
    parameters: String,
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
}

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    match language_from_path(&input.path).as_deref() {
        Some("csharp") => parse_csharp_file(input),
        Some("csproj") => parse_csproj_file(input),
        _ => NoopTreeSitterParser.parse(input),
    }
}

fn parse_csharp_file(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input)];
    collect_namespace_symbols(&input, &mut symbols);
    collect_using_symbols(&input, &mut symbols);

    let classes = collect_classes(&input);
    for class in &classes {
        let controller = controller_metadata(class);
        symbols.push(class_symbol(&input, class, controller.as_deref()));
        collect_class_members(&input, class, &mut symbols);
    }
    symbols.extend(data_access::collect_csharp_data_access(&input, &symbols));
    symbols.extend(realtime::collect_csharp_realtime(&input, &symbols));

    let relationships = collect_csharp_relationships(&symbols);
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("csharp".to_string()),
        symbols,
        relationships,
    })
}

fn parse_csproj_file(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut technologies = detect_csproj_technologies(&input.source)?;
    technologies.extend(data_access::detect_csproj_data_access_technologies(
        &input.source,
    )?);
    let mut symbols = vec![module_symbol(&input)];
    for technology in technologies {
        symbols.push(ExtractedSymbol {
            id: SymbolId::new(stable_id(
                "symbol",
                &format!("{}:csproj:{}", input.file_id.as_str(), technology.id),
            )),
            file_id: input.file_id.clone(),
            name: technology.name,
            kind: NodeKind::Package,
            start_byte: 0,
            end_byte: input.source.len(),
            start_line: 1,
            start_column: 0,
            end_line: input.source.lines().count().max(1),
            end_column: input.source.lines().last().unwrap_or_default().len(),
            visibility: Some(format!(
                "dotnet.project=true;dotnet.technology={};dotnet.support={:?};dotnet.source={}",
                technology.id, technology.support_level, technology.source
            )),
        });
    }

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("csproj".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

pub fn detect_csproj_technologies(source: &str) -> ContractResult<Vec<DetectedTechnology>> {
    let mut detected = Vec::new();
    let lower = source.to_ascii_lowercase();
    if lower.contains("microsoft.net.sdk.web")
        || lower.contains("microsoft.aspnetcore.app")
        || lower.contains("microsoft.aspnetcore.mvc")
        || lower.contains("microsoft.aspnetcore.mvc.core")
        || lower.contains("packagereference include=\"microsoft.aspnetcore.")
        || lower.contains("frameworkreference include=\"microsoft.aspnetcore.app\"")
    {
        detected.push(DetectedTechnology {
            id: "aspnetcore".to_string(),
            name: "ASP.NET Core".to_string(),
            kind: TechnologyKind::WebBackend,
            support_level: TechnologySupportLevel::Basic,
            capabilities: vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::ExtractRoutes,
            ],
            source: ".csproj".to_string(),
        });
    }
    if lower.contains("<targetframework") {
        detected.push(DetectedTechnology {
            id: "dotnet".to_string(),
            name: ".NET".to_string(),
            kind: TechnologyKind::Runtime,
            support_level: TechnologySupportLevel::DetectOnly,
            capabilities: vec![TechnologyCapability::DetectPackage],
            source: ".csproj".to_string(),
        });
    }
    for technology in realtime::detect_csproj_realtime_technologies(source)? {
        if !detected
            .iter()
            .any(|existing: &DetectedTechnology| existing.id == technology.id)
        {
            detected.push(technology);
        }
    }
    Ok(detected)
}

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:csharp-module:{}",
                input.file_id.as_str(),
                input.path.display()
            ),
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
        end_line: input.source.lines().count().max(1),
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: None,
    }
}

fn collect_namespace_symbols(input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    for (line_index, line) in input.source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("namespace ") {
            let name = rest
                .split([';', '{', ' ', '\t'])
                .next()
                .unwrap_or_default()
                .trim();
            if !name.is_empty() {
                symbols.push(simple_line_symbol(
                    input,
                    name,
                    NodeKind::Namespace,
                    line_index + 1,
                    "csharp.namespace=true",
                ));
            }
        }
    }
}

fn collect_using_symbols(input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    for (line_index, line) in input.source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("using ") {
            let name = rest.trim_end_matches(';').trim();
            if !name.is_empty() && !name.contains('=') && !name.contains('(') {
                symbols.push(simple_line_symbol(
                    input,
                    name,
                    NodeKind::Package,
                    line_index + 1,
                    "csharp.using=true",
                ));
            }
        }
    }
}

fn collect_classes(input: &ParseInput) -> Vec<ClassInfo> {
    let line_offsets = line_offsets(&input.source);
    let lines: Vec<&str> = input.source.lines().collect();
    let mut classes = Vec::new();
    let mut pending_attrs = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(attr) = parse_attribute_line(trimmed, index + 1) {
            pending_attrs.push(attr);
            continue;
        }
        if let Some((name, base_text)) = parse_class_declaration(trimmed) {
            let start_byte = line_offsets[index];
            let end_byte =
                find_block_end(&input.source, start_byte).unwrap_or(start_byte + line.len());
            let end_line = byte_to_line(&line_offsets, end_byte);
            classes.push(ClassInfo {
                name,
                base_text,
                attrs: std::mem::take(&mut pending_attrs),
                start_byte,
                end_byte,
                start_line: index + 1,
                end_line,
            });
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            pending_attrs.clear();
        }
    }
    classes
}

fn collect_class_members(
    input: &ParseInput,
    class: &ClassInfo,
    symbols: &mut Vec<ExtractedSymbol>,
) {
    let body = input
        .source
        .get(class.start_byte..class.end_byte)
        .unwrap_or_default();
    let base_line = class.start_line.saturating_sub(1);
    let methods = collect_methods(body, base_line, class.start_byte, &class.name);
    let dependencies = methods
        .iter()
        .find(|method| method.name == class.name)
        .map(|method| constructor_dependency_types(&method.parameters))
        .unwrap_or_default();

    if !dependencies.is_empty() {
        if let Some(class_symbol) = symbols.iter_mut().find(|symbol| {
            symbol.kind == NodeKind::Class
                && symbol.name == class.name
                && symbol.start_byte == class.start_byte
        }) {
            let metadata = class_symbol.visibility.get_or_insert_with(String::new);
            append_metadata(metadata, "aspnet.dependencies", &dependencies.join(","));
        }
    }

    for method in methods {
        let is_constructor = method.name == class.name;
        symbols.push(method_symbol(input, class, &method, is_constructor));
        if !is_constructor {
            collect_action_route(input, class, &method, symbols);
        }
    }
}

fn collect_methods(
    body: &str,
    base_line: usize,
    base_byte: usize,
    class_name: &str,
) -> Vec<MethodInfo> {
    let lines: Vec<&str> = body.lines().collect();
    let offsets = line_offsets(body);
    let mut pending_attrs = Vec::new();
    let mut methods = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(attr) = parse_attribute_line(trimmed, base_line + index + 1) {
            pending_attrs.push(attr);
            continue;
        }
        if let Some((name, parameters)) = parse_public_method(trimmed, class_name) {
            let start_byte = base_byte + offsets[index];
            let end_byte = find_block_end(body, offsets[index])
                .map(|offset| base_byte + offset)
                .unwrap_or(start_byte + line.len());
            methods.push(MethodInfo {
                name,
                attrs: std::mem::take(&mut pending_attrs),
                parameters,
                start_byte,
                end_byte,
                start_line: base_line + index + 1,
                end_line: base_line + byte_to_line(&offsets, end_byte.saturating_sub(base_byte)),
            });
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            pending_attrs.clear();
        }
    }
    methods
}

fn collect_action_route(
    input: &ParseInput,
    class: &ClassInfo,
    method: &MethodInfo,
    symbols: &mut Vec<ExtractedSymbol>,
) {
    let Some((http_method, method_route, source_kind, confidence)) = route_attribute(&method.attrs)
    else {
        return;
    };
    let controller_route = route_template(&class.attrs).unwrap_or_default();
    let Some(path) = compose_route(
        &controller_route,
        method_route.as_deref().unwrap_or_default(),
        &class.name,
        &method.name,
    ) else {
        return;
    };
    let metadata = RouteMetadata {
        framework: "aspnetcore".to_string(),
        route_kind: "api".to_string(),
        method: http_method,
        path,
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: Some(method.name.clone()),
        class_name: Some(class.name.clone()),
        function_name: Some(method.name.clone()),
        line_start: method.start_line,
        line_end: method.end_line,
        confidence,
        source_kind,
    };
    symbols.push(ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:aspnet-route:{}:{}:{}",
                input.file_id.as_str(),
                metadata.method,
                metadata.path,
                method.start_byte
            ),
        )),
        file_id: input.file_id.clone(),
        name: format!("{} {}", metadata.method, metadata.path),
        kind: NodeKind::Route,
        start_byte: method.start_byte,
        end_byte: method.end_byte,
        start_line: method.start_line,
        start_column: 0,
        end_line: method.end_line,
        end_column: 0,
        visibility: Some(encode_route_metadata(&metadata)),
    });
}

fn collect_csharp_relationships(symbols: &[ExtractedSymbol]) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    for route in symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
    {
        let Some(metadata) = route.visibility.as_deref() else {
            continue;
        };
        let Some(handler) = route_metadata_value(metadata, "handler") else {
            continue;
        };
        if let Some(target) = symbols.iter().find(|symbol| {
            symbol.kind == NodeKind::Method && symbol.name == handler && symbol.id != route.id
        }) {
            relationships.push(index_edge(
                &route.id,
                &target.id,
                EdgeKind::References,
                EdgeProvenance::TextHeuristic,
                8_500,
            ));
        }
    }
    relationships
}

fn class_symbol(input: &ParseInput, class: &ClassInfo, metadata: Option<&str>) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:csharp-class:{}:{}",
                input.file_id.as_str(),
                class.name,
                class.start_byte
            ),
        )),
        file_id: input.file_id.clone(),
        name: class.name.clone(),
        kind: NodeKind::Class,
        start_byte: class.start_byte,
        end_byte: class.end_byte,
        start_line: class.start_line,
        start_column: 0,
        end_line: class.end_line,
        end_column: 0,
        visibility: metadata.map(str::to_string),
    }
}

fn method_symbol(
    input: &ParseInput,
    class: &ClassInfo,
    method: &MethodInfo,
    is_constructor: bool,
) -> ExtractedSymbol {
    let mut visibility = if is_constructor {
        "csharp.constructor=true".to_string()
    } else {
        "csharp.method=true".to_string()
    };
    if method.attrs.iter().any(is_http_attribute) {
        append_metadata(&mut visibility, "aspnet.action", "true");
    }
    append_metadata(&mut visibility, "csharp.class", &class.name);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:csharp-method:{}:{}:{}",
                input.file_id.as_str(),
                class.name,
                method.name,
                method.start_byte
            ),
        )),
        file_id: input.file_id.clone(),
        name: method.name.clone(),
        kind: NodeKind::Method,
        start_byte: method.start_byte,
        end_byte: method.end_byte,
        start_line: method.start_line,
        start_column: 0,
        end_line: method.end_line,
        end_column: 0,
        visibility: Some(visibility),
    }
}

fn simple_line_symbol(
    input: &ParseInput,
    name: &str,
    kind: NodeKind,
    line: usize,
    visibility: &str,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:{kind:?}:{name}:{line}", input.file_id.as_str()),
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
        visibility: Some(visibility.to_string()),
    }
}

fn controller_metadata(class: &ClassInfo) -> Option<String> {
    let is_api_controller = has_attribute(&class.attrs, "ApiController");
    let route = route_template(&class.attrs);
    let inherits_controller = class.base_text.as_deref().is_some_and(|base| {
        base.contains("ControllerBase")
            || base
                .split([',', ' ', '\t'])
                .any(|part| part == "Controller")
    });
    let controller = class.name.ends_with("Controller")
        || is_api_controller
        || route.is_some()
        || inherits_controller;
    if !controller {
        return None;
    }
    let mut metadata = "aspnet.controller=true".to_string();
    append_metadata(&mut metadata, "aspnet.framework", "aspnetcore");
    append_metadata(
        &mut metadata,
        "aspnet.api_controller",
        bool_str(is_api_controller),
    );
    if let Some(route) = route {
        append_metadata(&mut metadata, "aspnet.route", &route);
    }
    if let Some(base) = &class.base_text {
        append_metadata(&mut metadata, "aspnet.base", base);
    }
    Some(metadata)
}

fn parse_attribute_line(line: &str, _line_number: usize) -> Option<AttributeInfo> {
    let text = line.strip_prefix('[')?.rsplit_once(']')?.0.trim();
    let name_end = text.find(['(', ' ', '\t']).unwrap_or(text.len());
    let name = text[..name_end]
        .trim()
        .trim_end_matches("Attribute")
        .to_string();
    if name.is_empty() {
        return None;
    }
    let argument = text
        .find('(')
        .and_then(|start| text.rfind(')').map(|end| (start, end)))
        .and_then(|(start, end)| literal_string(text.get(start + 1..end).unwrap_or_default()));
    Some(AttributeInfo { name, argument })
}

fn parse_class_declaration(line: &str) -> Option<(String, Option<String>)> {
    let marker = " class ";
    let class_pos = line
        .find(marker)
        .or_else(|| line.strip_prefix("class ").map(|_| 0))?;
    let after = if class_pos == 0 && line.starts_with("class ") {
        &line["class ".len()..]
    } else {
        &line[class_pos + marker.len()..]
    };
    let name = after
        .split([' ', '\t', ':', '{', '('])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let base_text = after
        .split_once(':')
        .map(|(_, right)| {
            right
                .split('{')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty());
    Some((name, base_text))
}

fn parse_public_method(line: &str, class_name: &str) -> Option<(String, String)> {
    if !line.contains('(')
        || !line.contains(')')
        || line.contains(" class ")
        || line.starts_with("if ")
        || line.starts_with("for ")
        || line.starts_with("while ")
        || !line.split_whitespace().any(|part| part == "public")
    {
        return None;
    }
    let before_paren = line.split_once('(')?.0.trim();
    let name = before_paren
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    if !is_csharp_identifier(&name) {
        return None;
    }
    let params = line
        .split_once('(')?
        .1
        .split_once(')')
        .map(|(params, _)| params.trim().to_string())
        .unwrap_or_default();
    if name == class_name || before_paren.split_whitespace().count() >= 2 {
        return Some((name, params));
    }
    None
}

fn route_attribute(attrs: &[AttributeInfo]) -> Option<(String, Option<String>, String, u16)> {
    for attr in attrs {
        let name = attr.name.as_str();
        let method = match name {
            "HttpGet" => "GET",
            "HttpPost" => "POST",
            "HttpPut" => "PUT",
            "HttpPatch" => "PATCH",
            "HttpDelete" => "DELETE",
            "HttpHead" => "HEAD",
            "HttpOptions" => "OPTIONS",
            _ => continue,
        };
        return Some((
            method.to_string(),
            attr.argument.clone(),
            format!("AspNetCore{name}Attribute"),
            9_500,
        ));
    }
    attrs.iter().find(|attr| attr.name == "Route").map(|attr| {
        (
            "UNKNOWN".to_string(),
            attr.argument.clone(),
            "AspNetCoreRouteAttribute".to_string(),
            7_500,
        )
    })
}

fn compose_route(base: &str, path: &str, class_name: &str, action_name: &str) -> Option<String> {
    if base.is_empty() && path.is_empty() {
        return None;
    }
    let controller = class_name
        .strip_suffix("Controller")
        .unwrap_or(class_name)
        .to_ascii_lowercase();
    let clean_base = replace_route_tokens(base, &controller, action_name);
    let clean_path = replace_route_tokens(path, &controller, action_name);
    Some(normalize_route_path(&clean_base, &clean_path))
}

fn replace_route_tokens(value: &str, controller: &str, action: &str) -> String {
    value
        .replace("[controller]", controller)
        .replace("[Controller]", controller)
        .replace("[action]", action)
        .replace("[Action]", action)
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

fn route_template(attrs: &[AttributeInfo]) -> Option<String> {
    attrs
        .iter()
        .find(|attr| attr.name == "Route")
        .and_then(|attr| attr.argument.clone())
}

fn has_attribute(attrs: &[AttributeInfo], name: &str) -> bool {
    attrs.iter().any(|attr| attr.name == name)
}

fn is_http_attribute(attr: &AttributeInfo) -> bool {
    matches!(
        attr.name.as_str(),
        "HttpGet"
            | "HttpPost"
            | "HttpPut"
            | "HttpPatch"
            | "HttpDelete"
            | "HttpHead"
            | "HttpOptions"
    )
}

fn constructor_dependency_types(parameters: &str) -> Vec<String> {
    parameters
        .split(',')
        .filter_map(|param| {
            let trimmed = param.trim();
            if trimmed.is_empty() {
                return None;
            }
            let without_default = trimmed.split('=').next().unwrap_or(trimmed).trim();
            let parts: Vec<&str> = without_default.split_whitespace().collect();
            if parts.len() < 2 {
                return None;
            }
            Some(parts[..parts.len() - 1].join(" "))
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn literal_string(arguments: &str) -> Option<String> {
    let start = arguments.find('"')?;
    let rest = &arguments[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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

fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("route.{key}="))
            .map(|value| value.replace("%3B", ";"))
    })
}

#[cfg(test)]
pub(crate) fn aspnet_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("aspnet.{key}="))
            .map(|value| value.replace("%3B", ";"))
    })
}

fn append_metadata(metadata: &mut String, key: &str, value: &str) {
    if !metadata.is_empty() {
        metadata.push(';');
    }
    metadata.push_str(key);
    metadata.push('=');
    metadata.push_str(&value.replace(';', "%3B"));
}

fn bool_str(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn is_csharp_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
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
