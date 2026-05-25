use super::*;

mod rsocket;
mod signalr;
mod socketio;
mod websocket;

pub(crate) fn collect_web_realtime(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    output.extend(websocket::collect_websocket(input, symbols));
    output.extend(socketio::collect_socketio(input, symbols));
    output.extend(signalr::collect_signalr_client(input, symbols));
    output.extend(rsocket::collect_rsocket(input, symbols));
    output
}

pub(crate) fn collect_csharp_realtime(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    signalr::collect_signalr_csharp(input, symbols)
}

pub fn detect_package_json_realtime_technologies(
    source: &str,
) -> ContractResult<Vec<DetectedTechnology>> {
    let value = serde_json::from_str::<serde_json::Value>(source)
        .map_err(|error| ContractError::new(format!("invalid package.json: {error}")))?;
    let mut detected = Vec::new();
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        let Some(object) = value.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for package_name in object.keys() {
            let Some((id, name, support_level)) = package_realtime(package_name) else {
                continue;
            };
            if detected
                .iter()
                .any(|tech: &DetectedTechnology| tech.id == id)
            {
                continue;
            }
            detected.push(realtime_technology(
                id,
                name,
                support_level,
                &format!("package.json:{section}:{package_name}"),
            ));
        }
    }
    Ok(detected)
}

pub fn detect_csproj_realtime_technologies(
    source: &str,
) -> ContractResult<Vec<DetectedTechnology>> {
    let lower = source.to_ascii_lowercase();
    let mut detected = Vec::new();
    if lower.contains("microsoft.aspnetcore.signalr") {
        detected.push(realtime_technology(
            "signalr",
            "SignalR",
            TechnologySupportLevel::Basic,
            "csproj",
        ));
    }
    Ok(detected)
}

fn package_realtime(
    package_name: &str,
) -> Option<(&'static str, &'static str, TechnologySupportLevel)> {
    match package_name {
        "ws" | "websocket" => Some(("websocket", "WebSocket", TechnologySupportLevel::Basic)),
        "socket.io" | "socket.io-client" => {
            Some(("socketio", "Socket.IO", TechnologySupportLevel::Basic))
        }
        "@microsoft/signalr" => Some(("signalr", "SignalR", TechnologySupportLevel::Basic)),
        "rsocket-core" | "rsocket-websocket-client" | "rsocket-tcp-client" => {
            Some(("rsocket", "RSocket", TechnologySupportLevel::Basic))
        }
        _ => None,
    }
}

fn realtime_technology(
    id: &str,
    name: &str,
    support_level: TechnologySupportLevel,
    source: &str,
) -> DetectedTechnology {
    DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind: TechnologyKind::Realtime,
        support_level,
        capabilities: vec![
            TechnologyCapability::DetectPackage,
            TechnologyCapability::DetectImport,
            TechnologyCapability::ExtractRealtime,
        ],
        source: source.to_string(),
    }
}

pub(crate) fn realtime_symbol(
    input: &ParseInput,
    line: usize,
    name: &str,
    metadata: RealtimeMetadata,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:realtime:{}:{}:{}:{}",
                input.file_id.as_str(),
                metadata.technology,
                metadata.source_kind,
                name,
                line
            ),
        )),
        file_id: input.file_id.clone(),
        name: name.to_string(),
        kind: NodeKind::Endpoint,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: line,
        end_column: 0,
        visibility: Some(encode_realtime_metadata(&metadata)),
    }
}

pub(crate) fn metadata(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    line: usize,
    technology: &str,
    kind: &str,
    direction: &str,
    source_kind: &str,
) -> RealtimeMetadata {
    RealtimeMetadata {
        technology: technology.to_string(),
        kind: kind.to_string(),
        direction: direction.to_string(),
        event_name: None,
        channel_name: None,
        hub_name: None,
        method_name: containing_method_name(symbols, line),
        endpoint: None,
        file_path: normalized_file(input),
        symbol_id: containing_symbol_id(symbols, line),
        class_name: containing_class_name(symbols, line),
        function_name: containing_method_name(symbols, line),
        line_start: line,
        line_end: line,
        confidence: 8_000,
        source_kind: source_kind.to_string(),
    }
}

pub(crate) fn encode_realtime_metadata(metadata: &RealtimeMetadata) -> String {
    [
        ("realtime.technology", Some(metadata.technology.as_str())),
        ("realtime.kind", Some(metadata.kind.as_str())),
        ("realtime.direction", Some(metadata.direction.as_str())),
        ("realtime.event", metadata.event_name.as_deref()),
        ("realtime.channel", metadata.channel_name.as_deref()),
        ("realtime.hub", metadata.hub_name.as_deref()),
        ("realtime.method", metadata.method_name.as_deref()),
        ("realtime.endpoint", metadata.endpoint.as_deref()),
        ("realtime.file", Some(metadata.file_path.as_str())),
        ("realtime.class", metadata.class_name.as_deref()),
        ("realtime.function", metadata.function_name.as_deref()),
        ("realtime.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("realtime.line_start={}", metadata.line_start),
        format!("realtime.line_end={}", metadata.line_end),
        format!("realtime.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

pub(crate) fn literal_string_argument(line: &str) -> Option<String> {
    let start = line.find(['"', '\''])?;
    let quote = line.as_bytes().get(start).copied()? as char;
    let rest = &line[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(crate) fn call_literal_argument(line: &str, call: &str) -> Option<String> {
    let start = line.find(call)? + call.len();
    literal_string_argument(line.get(start..)?)
}

pub(crate) fn has_import_or_require(line: &str, package: &str) -> bool {
    line.contains(&format!("\"{package}\""))
        || line.contains(&format!("'{package}'"))
        || line.contains(&format!("from {package}"))
}

pub(crate) fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}

fn containing_symbol_id(symbols: &[ExtractedSymbol], line: usize) -> Option<SymbolId> {
    containing_symbol(
        symbols,
        line,
        &[NodeKind::Method, NodeKind::Function, NodeKind::Class],
    )
    .map(|symbol| symbol.id.clone())
}

fn containing_method_name(symbols: &[ExtractedSymbol], line: usize) -> Option<String> {
    containing_symbol(symbols, line, &[NodeKind::Method, NodeKind::Function])
        .map(|symbol| symbol.name.clone())
}

fn containing_class_name(symbols: &[ExtractedSymbol], line: usize) -> Option<String> {
    containing_symbol(symbols, line, &[NodeKind::Class]).map(|symbol| symbol.name.clone())
}

fn containing_symbol<'a>(
    symbols: &'a [ExtractedSymbol],
    line: usize,
    kinds: &[NodeKind],
) -> Option<&'a ExtractedSymbol> {
    symbols
        .iter()
        .filter(|symbol| kinds.contains(&symbol.kind))
        .filter(|symbol| symbol.start_line <= line && symbol.end_line >= line)
        .min_by_key(|symbol| symbol.end_line.saturating_sub(symbol.start_line))
}

fn escape_metadata(value: &str) -> String {
    value.replace(';', "%3B").replace('\n', "\\n")
}

#[cfg(test)]
fn unescape_metadata(value: &str) -> String {
    value.replace("%3B", ";").replace("\\n", "\n")
}

#[cfg(test)]
pub(crate) fn realtime_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("realtime.{key}="))
            .map(unescape_metadata)
    })
}
