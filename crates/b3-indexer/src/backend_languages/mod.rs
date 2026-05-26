use super::*;

mod java;
mod kotlin;
mod php;
mod python;
mod ruby;

#[derive(Debug, Clone)]
pub(crate) struct BackendSymbol {
    pub name: String,
    pub kind: NodeKind,
    pub line: usize,
    pub metadata: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BackendRoute {
    pub framework: &'static str,
    pub method: String,
    pub path: String,
    pub handler: Option<String>,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub line: usize,
    pub source_kind: &'static str,
    pub confidence: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct BackendDataAccess {
    pub technology: &'static str,
    pub kind: &'static str,
    pub operation: Option<String>,
    pub entity_name: Option<String>,
    pub repository_name: Option<String>,
    pub query_text: Option<String>,
    pub class_name: Option<String>,
    pub method_name: Option<String>,
    pub line: usize,
    pub source_kind: &'static str,
    pub confidence: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct BackendMessaging {
    pub technology: &'static str,
    pub kind: &'static str,
    pub direction: &'static str,
    pub topic: Option<String>,
    pub queue: Option<String>,
    pub exchange: Option<String>,
    pub routing_key: Option<String>,
    pub pattern: Option<String>,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub method_name: Option<String>,
    pub line: usize,
    pub source_kind: &'static str,
    pub confidence: u16,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BackendParseResult {
    pub language: &'static str,
    pub symbols: Vec<BackendSymbol>,
    pub routes: Vec<BackendRoute>,
    pub data_access: Vec<BackendDataAccess>,
    pub messaging: Vec<BackendMessaging>,
}

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let Some(language) = language_from_path(&input.path) else {
        return NoopTreeSitterParser.parse(input);
    };

    let result = match language.as_str() {
        "python" | "python_project" => python::parse(&input),
        "java" | "java_project" => java::parse(&input),
        "kotlin" | "kotlin_project" => kotlin::parse(&input),
        "php" | "php_project" => php::parse(&input),
        "ruby" | "ruby_project" => ruby::parse(&input),
        _ => return NoopTreeSitterParser.parse(input),
    };

    let mut symbols = vec![module_symbol(&input, result.language)];
    symbols.extend(
        result
            .symbols
            .into_iter()
            .map(|symbol| backend_symbol(&input, result.language, symbol)),
    );
    for route in result.routes {
        symbols.push(route_symbol(&input, route));
    }
    for data_access in result.data_access {
        symbols.push(data_access_symbol(&input, data_access));
    }
    for messaging in result.messaging {
        symbols.push(messaging_symbol(&input, messaging));
    }

    let mut relationships = Vec::new();
    collect_contains_relationships(&symbols, &mut relationships);
    collect_import_relationships(&symbols, &mut relationships);
    collect_route_handler_relationships(&symbols, &mut relationships);

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some(result.language.to_string()),
        symbols,
        relationships,
    })
}

pub(crate) fn module_symbol(input: &ParseInput, language: &str) -> ExtractedSymbol {
    let end_line = input.source.lines().count().max(1);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}-module:{}",
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
        end_line,
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some(format!("{language}.file=true")),
    }
}

pub(crate) fn project_technology_symbols(
    input: &ParseInput,
    language: &str,
    detected: Vec<DetectedTechnology>,
) -> Vec<BackendSymbol> {
    detected
        .into_iter()
        .map(|technology| BackendSymbol {
            name: technology.name,
            kind: NodeKind::Package,
            line: 1,
            metadata: format!(
                "{language}.technology={};{language}.support={:?};{language}.source={}",
                technology.id, technology.support_level, technology.source
            ),
        })
        .chain(std::iter::once(BackendSymbol {
            name: input
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("backend-project")
                .to_string(),
            kind: NodeKind::Package,
            line: 1,
            metadata: format!("{language}.project=true;{language}.support=Basic"),
        }))
        .collect()
}

fn backend_symbol(input: &ParseInput, language: &str, symbol: BackendSymbol) -> ExtractedSymbol {
    let (start_byte, end_byte, end_line, end_column) = line_span(input, symbol.line);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}:{}:{}:{}",
                input.file_id.as_str(),
                node_kind_name(symbol.kind),
                symbol.name,
                symbol.line
            ),
        )),
        file_id: input.file_id.clone(),
        name: symbol.name,
        kind: symbol.kind,
        start_byte,
        end_byte,
        start_line: symbol.line,
        start_column: 0,
        end_line,
        end_column,
        visibility: Some(symbol.metadata),
    }
}

fn route_symbol(input: &ParseInput, route: BackendRoute) -> ExtractedSymbol {
    let (start_byte, end_byte, end_line, end_column) = line_span(input, route.line);
    let metadata = RouteMetadata {
        framework: route.framework.to_string(),
        route_kind: "HttpRoute".to_string(),
        method: route.method.to_ascii_uppercase(),
        path: normalize_route_path("", &route.path),
        file_path: normalized_file(input),
        symbol_id: None,
        handler_name: route.handler.clone(),
        class_name: route.class_name,
        function_name: route.function_name,
        line_start: route.line,
        line_end: route.line,
        confidence: route.confidence,
        source_kind: route.source_kind.to_string(),
    };
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:route:{}:{}:{}:{}",
                input.file_id.as_str(),
                metadata.framework,
                metadata.method,
                metadata.path,
                route.line
            ),
        )),
        file_id: input.file_id.clone(),
        name: format!("{} {}", metadata.method, metadata.path),
        kind: NodeKind::Route,
        start_byte,
        end_byte,
        start_line: route.line,
        start_column: 0,
        end_line,
        end_column,
        visibility: Some(encode_route_metadata(&metadata)),
    }
}

fn data_access_symbol(input: &ParseInput, metadata: BackendDataAccess) -> ExtractedSymbol {
    let (start_byte, end_byte, end_line, end_column) = line_span(input, metadata.line);
    let encoded = encode_data_access_metadata(&DataAccessMetadata {
        technology: metadata.technology.to_string(),
        kind: metadata.kind.to_string(),
        file_path: normalized_file(input),
        symbol_id: None,
        class_name: metadata.class_name,
        method_name: metadata.method_name,
        entity_name: metadata.entity_name,
        context_name: None,
        repository_name: metadata.repository_name,
        operation: metadata.operation,
        query_text: metadata.query_text,
        line_start: metadata.line,
        line_end: metadata.line,
        confidence: metadata.confidence,
        source_kind: metadata.source_kind.to_string(),
    });
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:data-access:{}:{}:{}",
                input.file_id.as_str(),
                metadata.technology,
                metadata.source_kind,
                metadata.line
            ),
        )),
        file_id: input.file_id.clone(),
        name: format!("{} {}", metadata.technology, metadata.kind),
        kind: NodeKind::Endpoint,
        start_byte,
        end_byte,
        start_line: metadata.line,
        start_column: 0,
        end_line,
        end_column,
        visibility: Some(encoded),
    }
}

fn messaging_symbol(input: &ParseInput, metadata: BackendMessaging) -> ExtractedSymbol {
    let (start_byte, end_byte, end_line, end_column) = line_span(input, metadata.line);
    let encoded = encode_messaging_metadata(&MessagingMetadata {
        technology: metadata.technology.to_string(),
        kind: metadata.kind.to_string(),
        direction: metadata.direction.to_string(),
        topic: metadata.topic,
        queue: metadata.queue,
        exchange: metadata.exchange,
        routing_key: metadata.routing_key,
        pattern: metadata.pattern,
        consumer_group: None,
        file_path: normalized_file(input),
        symbol_id: None,
        class_name: metadata.class_name,
        function_name: metadata.function_name,
        method_name: metadata.method_name,
        line_start: metadata.line,
        line_end: metadata.line,
        confidence: metadata.confidence,
        source_kind: metadata.source_kind.to_string(),
    });
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:messaging:{}:{}:{}",
                input.file_id.as_str(),
                metadata.technology,
                metadata.source_kind,
                metadata.line
            ),
        )),
        file_id: input.file_id.clone(),
        name: format!("{} {}", metadata.technology, metadata.kind),
        kind: NodeKind::Endpoint,
        start_byte,
        end_byte,
        start_line: metadata.line,
        start_column: 0,
        end_line,
        end_column,
        visibility: Some(encoded),
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

pub(crate) fn detect_dependency(source: &str, needles: &[&str]) -> bool {
    let lower = source.to_ascii_lowercase();
    needles
        .iter()
        .any(|needle| lower.contains(&needle.to_ascii_lowercase()))
}

pub(crate) fn technology(
    id: &str,
    name: &str,
    kind: TechnologyKind,
    support_level: TechnologySupportLevel,
    capabilities: Vec<TechnologyCapability>,
    source: impl Into<String>,
) -> DetectedTechnology {
    DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        support_level,
        capabilities,
        source: source.into(),
    }
}

pub(crate) fn literal_after(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    literal_in(line.get(start..)?)
}

pub(crate) fn literal_in(value: &str) -> Option<String> {
    let start = value.find(['"', '\''])?;
    let quote = value.as_bytes().get(start).copied()? as char;
    let rest = &value[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(crate) fn annotation_literal(line: &str, annotation: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(annotation)?;
    literal_in(rest).or_else(|| {
        rest.find("value")
            .and_then(|_| literal_after(rest, "value"))
            .or_else(|| rest.find("path").and_then(|_| literal_after(rest, "path")))
    })
}

pub(crate) fn route_method_from_annotation(line: &str) -> Option<(&'static str, String)> {
    let trimmed = line.trim();
    for (annotation, method) in [
        ("@GetMapping", "GET"),
        ("@PostMapping", "POST"),
        ("@PutMapping", "PUT"),
        ("@PatchMapping", "PATCH"),
        ("@DeleteMapping", "DELETE"),
        ("@GET", "GET"),
        ("@POST", "POST"),
        ("@PUT", "PUT"),
        ("@PATCH", "PATCH"),
        ("@DELETE", "DELETE"),
    ] {
        if trimmed.starts_with(annotation) {
            return Some((
                method,
                annotation_literal(trimmed, annotation).unwrap_or_default(),
            ));
        }
    }
    None
}

pub(crate) fn normalize_route_path(base: &str, path: &str) -> String {
    let clean_base = base.trim_matches('/');
    let clean_path = path.trim_matches('/');
    match (clean_base.is_empty(), clean_path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{clean_path}"),
        (false, true) => format!("/{clean_base}"),
        (false, false) => format!("/{clean_base}/{clean_path}"),
    }
}

pub(crate) fn current_class(symbols: &[BackendSymbol], line: usize) -> Option<String> {
    symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Class && symbol.line <= line)
        .next_back()
        .map(|symbol| symbol.name.clone())
}

pub(crate) fn current_function(symbols: &[BackendSymbol], line: usize) -> Option<String> {
    symbols
        .iter()
        .filter(|symbol| {
            matches!(symbol.kind, NodeKind::Function | NodeKind::Method) && symbol.line <= line
        })
        .next_back()
        .map(|symbol| symbol.name.clone())
}

pub(crate) fn identifier_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let start = line.find(marker)? + marker.len();
    line.get(start..)?
        .trim_start()
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|value| !value.is_empty())
}

pub(crate) fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}

fn line_span(input: &ParseInput, line: usize) -> (usize, usize, usize, usize) {
    let offsets = line_offsets(&input.source);
    let start = offsets.get(line.saturating_sub(1)).copied().unwrap_or(0);
    let end = offsets
        .get(line)
        .copied()
        .unwrap_or_else(|| input.source.len());
    let text = input
        .source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("");
    (start, end, line, text.len())
}

fn line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            offsets.push(index + 1);
        }
    }
    offsets
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

fn encode_data_access_metadata(metadata: &DataAccessMetadata) -> String {
    [
        ("data_access.technology", Some(metadata.technology.as_str())),
        ("data_access.kind", Some(metadata.kind.as_str())),
        ("data_access.file", Some(metadata.file_path.as_str())),
        ("data_access.class", metadata.class_name.as_deref()),
        ("data_access.method", metadata.method_name.as_deref()),
        ("data_access.entity", metadata.entity_name.as_deref()),
        ("data_access.context", metadata.context_name.as_deref()),
        (
            "data_access.repository",
            metadata.repository_name.as_deref(),
        ),
        ("data_access.operation", metadata.operation.as_deref()),
        ("data_access.query", metadata.query_text.as_deref()),
        ("data_access.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("data_access.line_start={}", metadata.line_start),
        format!("data_access.line_end={}", metadata.line_end),
        format!("data_access.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

fn encode_messaging_metadata(metadata: &MessagingMetadata) -> String {
    [
        ("messaging.technology", Some(metadata.technology.as_str())),
        ("messaging.kind", Some(metadata.kind.as_str())),
        ("messaging.direction", Some(metadata.direction.as_str())),
        ("messaging.topic", metadata.topic.as_deref()),
        ("messaging.queue", metadata.queue.as_deref()),
        ("messaging.exchange", metadata.exchange.as_deref()),
        ("messaging.routing_key", metadata.routing_key.as_deref()),
        ("messaging.pattern", metadata.pattern.as_deref()),
        (
            "messaging.consumer_group",
            metadata.consumer_group.as_deref(),
        ),
        ("messaging.file", Some(metadata.file_path.as_str())),
        ("messaging.class", metadata.class_name.as_deref()),
        ("messaging.function", metadata.function_name.as_deref()),
        ("messaging.method", metadata.method_name.as_deref()),
        ("messaging.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("messaging.line_start={}", metadata.line_start),
        format!("messaging.line_end={}", metadata.line_end),
        format!("messaging.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

#[cfg(test)]
pub(crate) fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata_value(metadata, "route", key)
}

#[cfg(not(test))]
fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata_value(metadata, "route", key)
}

#[cfg(test)]
pub(crate) fn backend_metadata_value(metadata: &str, prefix: &str, key: &str) -> Option<String> {
    metadata_value(metadata, prefix, key)
}

fn metadata_value(metadata: &str, prefix: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("{prefix}.{key}="))
            .map(unescape_metadata)
    })
}

fn escape_metadata(value: &str) -> String {
    value.replace(';', "%3B").replace('\n', "\\n")
}

fn unescape_metadata(value: &str) -> String {
    value.replace("%3B", ";").replace("\\n", "\n")
}
