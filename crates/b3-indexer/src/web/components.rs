use super::*;

pub(super) fn collect_component_relationships(
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

pub(super) fn encode_component_metadata(metadata: &ComponentMetadata) -> String {
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

pub(super) fn merge_visibility(existing: Option<String>, metadata: String) -> Option<String> {
    match existing {
        Some(existing) if !existing.is_empty() => Some(format!("{existing};{metadata}")),
        _ => Some(metadata),
    }
}
