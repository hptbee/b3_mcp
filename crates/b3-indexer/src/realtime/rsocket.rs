use super::*;

pub(super) fn collect_rsocket(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !rsocket_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "rsocket-core")
            || has_import_or_require(trimmed, "rsocket-websocket-client")
            || has_import_or_require(trimmed, "rsocket-tcp-client")
        {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "rsocket",
                "Connection",
                "bidirectional",
                "RSocketImport",
            );
            metadata.confidence = 7_500;
            output.push(realtime_symbol(
                input,
                line_number,
                "RSocket import",
                metadata,
            ));
        }
        for (call, source_kind) in [
            ("requestResponse", "RSocketRequestResponse"),
            ("fireAndForget", "RSocketFireAndForget"),
            ("requestStream", "RSocketRequestStream"),
            ("requestChannel", "RSocketRequestChannel"),
        ] {
            if trimmed.contains(&format!(".{call}(")) || trimmed.contains(&format!("{call}(")) {
                let mut metadata = metadata(
                    input,
                    symbols,
                    line_number,
                    "rsocket",
                    "Request",
                    "outbound",
                    source_kind,
                );
                metadata.method_name = Some(call.to_string());
                metadata.channel_name = route_metadata(trimmed);
                metadata.confidence = 7_500;
                output.push(realtime_symbol(
                    input,
                    line_number,
                    &format!("RSocket {call}"),
                    metadata,
                ));
            }
        }
    }
    output
}

fn rsocket_context(source: &str) -> bool {
    source.contains("rsocket-core")
        || source.contains("rsocket-websocket-client")
        || source.contains("rsocket-tcp-client")
        || source.contains("RSocketClient")
}

fn route_metadata(line: &str) -> Option<String> {
    if !(line.contains("route") || line.contains("metadata")) {
        return None;
    }
    literal_string_argument(line)
}
