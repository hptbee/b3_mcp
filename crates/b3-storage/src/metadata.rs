use super::*;

pub(super) fn graph_count_from_row(row: &Row<'_>) -> rusqlite::Result<GraphCount> {
    Ok(GraphCount {
        name: row.get(0)?,
        count: row.get::<_, i64>(1)? as usize,
    })
}

pub(super) fn graph_node_from_row(row: &Row<'_>) -> rusqlite::Result<StoredGraphNode> {
    Ok(StoredGraphNode {
        id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        name: row.get(3)?,
        kind: row.get(4)?,
        file_path: row.get(5)?,
        symbol_id: row.get(6)?,
        language: row.get(7)?,
        visibility: None,
        provenance: None,
    })
}

pub(super) fn graph_edge_from_row(row: &Row<'_>) -> rusqlite::Result<StoredGraphEdge> {
    Ok(StoredGraphEdge {
        id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        edge_type: row.get(3)?,
        from_node_id: row.get(4)?,
        to_node_id: row.get(5)?,
        confidence: row
            .get::<_, i64>(6)?
            .clamp(0, i64::from(EdgeConfidence::MAX_BASIS_POINTS)) as u16,
        provenance: row.get(7)?,
    })
}

pub(super) fn route_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRoute> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let name: String = row.get(3)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    let (fallback_method, fallback_path) = name
        .split_once(' ')
        .map(|(method, path)| (method.to_string(), path.to_string()))
        .unwrap_or_else(|| ("UNKNOWN".to_string(), name.clone()));
    Ok(StoredRoute {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        method: route_metadata_value(&metadata, "method").unwrap_or(fallback_method),
        path: route_metadata_value(&metadata, "path").unwrap_or(fallback_path),
        framework: route_metadata_value(&metadata, "framework")
            .unwrap_or_else(|| "unknown".to_string()),
        route_kind: route_metadata_value(&metadata, "kind").unwrap_or_else(|| "api".to_string()),
        file_path: route_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        handler_name: route_metadata_value(&metadata, "handler"),
        class_name: route_metadata_value(&metadata, "class"),
        function_name: route_metadata_value(&metadata, "function"),
        line_start: route_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: route_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: route_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: route_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

pub(super) fn component_from_row(row: &Row<'_>) -> rusqlite::Result<StoredComponent> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let name: String = row.get(3)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredComponent {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        name,
        framework: component_metadata_value(&metadata, "framework")
            .unwrap_or_else(|| "react".to_string()),
        file_path,
        symbol_id,
        export_kind: component_metadata_value(&metadata, "export"),
        component_kind: component_metadata_value(&metadata, "kind")
            .unwrap_or_else(|| "unknown".to_string()),
        props_type_name: component_metadata_value(&metadata, "props"),
        hooks: component_metadata_value(&metadata, "hooks")
            .map(|hooks| split_metadata_list(&hooks))
            .unwrap_or_default(),
        usages: component_metadata_value(&metadata, "usages")
            .map(|usages| split_metadata_list(&usages))
            .unwrap_or_default(),
        line_start: component_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: component_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: component_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: component_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

pub(super) fn data_access_from_row(row: &Row<'_>) -> rusqlite::Result<StoredDataAccess> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredDataAccess {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: data_access_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: data_access_metadata_value(&metadata, "kind")
            .unwrap_or_else(|| "Unknown".to_string()),
        operation: data_access_metadata_value(&metadata, "operation"),
        file_path: data_access_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        class_name: data_access_metadata_value(&metadata, "class"),
        method_name: data_access_metadata_value(&metadata, "method"),
        entity_name: data_access_metadata_value(&metadata, "entity"),
        context_name: data_access_metadata_value(&metadata, "context"),
        repository_name: data_access_metadata_value(&metadata, "repository"),
        query_text: data_access_metadata_value(&metadata, "query"),
        line_start: data_access_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: data_access_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: data_access_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: data_access_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

pub(super) fn realtime_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRealtime> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredRealtime {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: realtime_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: realtime_metadata_value(&metadata, "kind").unwrap_or_else(|| "Unknown".to_string()),
        direction: realtime_metadata_value(&metadata, "direction")
            .unwrap_or_else(|| "unknown".to_string()),
        event_name: realtime_metadata_value(&metadata, "event"),
        channel_name: realtime_metadata_value(&metadata, "channel"),
        hub_name: realtime_metadata_value(&metadata, "hub"),
        method_name: realtime_metadata_value(&metadata, "method"),
        endpoint: realtime_metadata_value(&metadata, "endpoint"),
        file_path: realtime_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        class_name: realtime_metadata_value(&metadata, "class"),
        function_name: realtime_metadata_value(&metadata, "function"),
        line_start: realtime_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: realtime_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: realtime_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: realtime_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

pub(super) fn messaging_from_row(row: &Row<'_>) -> rusqlite::Result<StoredMessaging> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredMessaging {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: messaging_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: messaging_metadata_value(&metadata, "kind").unwrap_or_else(|| "Unknown".to_string()),
        direction: messaging_metadata_value(&metadata, "direction")
            .unwrap_or_else(|| "unknown".to_string()),
        topic: messaging_metadata_value(&metadata, "topic"),
        queue: messaging_metadata_value(&metadata, "queue"),
        exchange: messaging_metadata_value(&metadata, "exchange"),
        routing_key: messaging_metadata_value(&metadata, "routing_key"),
        pattern: messaging_metadata_value(&metadata, "pattern"),
        consumer_group: messaging_metadata_value(&metadata, "consumer_group"),
        file_path: messaging_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        class_name: messaging_metadata_value(&metadata, "class"),
        function_name: messaging_metadata_value(&metadata, "function"),
        method_name: messaging_metadata_value(&metadata, "method"),
        line_start: messaging_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: messaging_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: messaging_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: messaging_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

pub(super) fn infrastructure_from_row(row: &Row<'_>) -> rusqlite::Result<StoredInfrastructure> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredInfrastructure {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: infrastructure_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: infrastructure_metadata_value(&metadata, "kind")
            .unwrap_or_else(|| "Unknown".to_string()),
        name: infrastructure_metadata_value(&metadata, "name"),
        resource_type: infrastructure_metadata_value(&metadata, "resource_type"),
        provider: infrastructure_metadata_value(&metadata, "provider"),
        image: infrastructure_metadata_value(&metadata, "image"),
        service_name: infrastructure_metadata_value(&metadata, "service_name"),
        container_name: infrastructure_metadata_value(&metadata, "container_name"),
        namespace: infrastructure_metadata_value(&metadata, "namespace"),
        ports: metadata_list(&metadata, "ports"),
        env_keys: metadata_list(&metadata, "env_keys"),
        labels: metadata_list(&metadata, "labels"),
        selectors: metadata_list(&metadata, "selectors"),
        file_path: infrastructure_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        line_start: infrastructure_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: infrastructure_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: infrastructure_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: infrastructure_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

pub(super) fn wpf_from_row(row: &Row<'_>) -> rusqlite::Result<StoredWpf> {
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let fallback_name: String = row.get(3)?;
    let symbol_id: String = row.get(4)?;
    let file_path: String = row.get(5)?;
    let fallback_line_start: usize = row.get(6)?;
    let fallback_line_end: usize = row.get(7)?;
    let metadata: String = row.get(8)?;

    Ok(StoredWpf {
        id,
        project_id,
        branch_id,
        technology: wpf_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "wpf".to_string()),
        kind: wpf_metadata_value(&metadata, "kind").unwrap_or_else(|| "Unknown".to_string()),
        name: wpf_metadata_value(&metadata, "name").or(Some(fallback_name)),
        x_class: wpf_metadata_value(&metadata, "x_class"),
        code_behind: wpf_metadata_value(&metadata, "code_behind"),
        view_model: wpf_metadata_value(&metadata, "view_model"),
        binding_paths: wpf_metadata_value(&metadata, "binding_paths")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        command_bindings: wpf_metadata_value(&metadata, "command_bindings")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        resource_keys: wpf_metadata_value(&metadata, "resource_keys")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        resource_sources: wpf_metadata_value(&metadata, "resource_sources")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        data_context: wpf_metadata_value(&metadata, "data_context"),
        file_path: wpf_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        line_start: wpf_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse().ok())
            .unwrap_or(fallback_line_start),
        line_end: wpf_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse().ok())
            .unwrap_or(fallback_line_end),
        confidence: wpf_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse().ok())
            .unwrap_or(7000),
        source_kind: wpf_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "WpfMetadata".to_string()),
    })
}

pub(super) fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("route.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

pub(super) fn infrastructure_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("infrastructure.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

pub(super) fn wpf_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("wpf.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.trim().to_string())
    })
}

pub(super) fn metadata_list(metadata: &str, key: &str) -> Vec<String> {
    infrastructure_metadata_value(metadata, key)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(super) fn component_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("component.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

pub(super) fn data_access_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("data_access.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

pub(super) fn realtime_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("realtime.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

pub(super) fn messaging_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("messaging.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

pub(super) fn split_metadata_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}
