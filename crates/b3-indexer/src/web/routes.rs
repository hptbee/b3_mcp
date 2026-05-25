use super::*;

pub(super) fn collect_route_handler_relationships(
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

pub(super) fn route_symbol(
    input: &ParseInput,
    node: Node<'_>,
    metadata: &RouteMetadata,
) -> ExtractedSymbol {
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

pub(super) fn normalize_route_path(base: &str, path: &str) -> String {
    let clean_base = base.trim_matches('/');
    let clean_path = path.trim_matches('/');
    match (clean_base.is_empty(), clean_path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{clean_path}"),
        (false, true) => format!("/{clean_base}"),
        (false, false) => format!("/{clean_base}/{clean_path}"),
    }
}

pub(super) fn encode_route_metadata(metadata: &RouteMetadata) -> String {
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
