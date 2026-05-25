use super::*;

pub(super) fn collect_signalr_client(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !signalr_client_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "@microsoft/signalr") {
            let metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Connection",
                "bidirectional",
                "SignalRClientImport",
            );
            output.push(realtime_symbol(
                input,
                line_number,
                "SignalR import",
                metadata,
            ));
        }
        if trimmed.contains("HubConnectionBuilder") || trimmed.contains(".withUrl(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Connection",
                "bidirectional",
                "SignalRHubConnectionBuilder",
            );
            metadata.endpoint = call_literal_argument(trimmed, ".withUrl(");
            metadata.confidence = 8_500;
            output.push(realtime_symbol(
                input,
                line_number,
                "SignalR client connection",
                metadata,
            ));
        }
        if let Some(event) = call_literal_argument(trimmed, ".on(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "ClientMethod",
                "inbound",
                "SignalRClientOn",
            );
            metadata.event_name = Some(event.clone());
            metadata.method_name = Some(event.clone());
            metadata.confidence = 9_000;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("SignalR on {event}"),
                metadata,
            ));
        }
        if let Some(method) = call_literal_argument(trimmed, ".invoke(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Emitter",
                "outbound",
                "SignalRClientInvoke",
            );
            metadata.method_name = Some(method.clone());
            metadata.confidence = 9_000;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("SignalR invoke {method}"),
                metadata,
            ));
        }
    }
    output
}

pub(super) fn collect_signalr_csharp(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    let lines: Vec<&str> = input.source.lines().collect();
    let hub_classes = hub_class_names(symbols);
    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "using Microsoft.AspNetCore.SignalR;" {
            let metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Connection",
                "bidirectional",
                "SignalRUsing",
            );
            output.push(realtime_symbol(
                input,
                line_number,
                "SignalR using",
                metadata,
            ));
        }
        if let Some(class_name) = hub_class_declaration(trimmed) {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Hub",
                "bidirectional",
                "SignalRHub",
            );
            metadata.hub_name = Some(class_name.clone());
            metadata.class_name = Some(class_name.clone());
            metadata.confidence = 9_500;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("SignalR hub {class_name}"),
                metadata,
            ));
        }
        if trimmed.contains("IHubContext<") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Hub",
                "bidirectional",
                "SignalRIHubContext",
            );
            metadata.hub_name = generic_argument(trimmed, "IHubContext<");
            metadata.confidence = 8_000;
            output.push(realtime_symbol(
                input,
                line_number,
                "SignalR hub context",
                metadata,
            ));
        }
        if let Some(method) = signalr_hub_method(symbols, line_number, &hub_classes) {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "HubMethod",
                "inbound",
                "SignalRHubMethod",
            );
            metadata.method_name = Some(method.clone());
            metadata.hub_name = containing_hub(symbols, line_number, &hub_classes);
            metadata.confidence = 8_500;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("SignalR hub method {method}"),
                metadata,
            ));
        }
        if let Some(event) = call_literal_argument(trimmed, ".SendAsync(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "signalr",
                "Emitter",
                "outbound",
                "SignalRSendAsync",
            );
            metadata.event_name = Some(event.clone());
            metadata.method_name = Some(event.clone());
            metadata.hub_name = containing_hub(symbols, line_number, &hub_classes);
            metadata.confidence = 9_000;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("SignalR send {event}"),
                metadata,
            ));
        }
    }
    output
}

fn signalr_client_context(source: &str) -> bool {
    source.contains("@microsoft/signalr") || source.contains("HubConnectionBuilder")
}

fn hub_class_declaration(line: &str) -> Option<String> {
    if !(line.contains(" class ") && line.contains(':') && line.contains("Hub")) {
        return None;
    }
    line.split(" class ")
        .nth(1)?
        .split([' ', ':', '{'])
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn hub_class_names(symbols: &[ExtractedSymbol]) -> Vec<String> {
    symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Class && symbol.name.ends_with("Hub"))
        .map(|symbol| symbol.name.clone())
        .collect()
}

fn signalr_hub_method(
    symbols: &[ExtractedSymbol],
    line: usize,
    hub_classes: &[String],
) -> Option<String> {
    if containing_hub(symbols, line, hub_classes).is_none() {
        return None;
    }
    symbols
        .iter()
        .find(|symbol| {
            symbol.kind == NodeKind::Method && symbol.start_line == line && symbol.name != "Hub"
        })
        .map(|symbol| symbol.name.clone())
}

fn containing_hub(
    symbols: &[ExtractedSymbol],
    line: usize,
    hub_classes: &[String],
) -> Option<String> {
    symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Class && hub_classes.contains(&symbol.name))
        .filter(|symbol| symbol.start_line <= line && symbol.end_line >= line)
        .min_by_key(|symbol| symbol.end_line.saturating_sub(symbol.start_line))
        .map(|symbol| symbol.name.clone())
}

fn generic_argument(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    line.get(start..)?
        .split('>')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
