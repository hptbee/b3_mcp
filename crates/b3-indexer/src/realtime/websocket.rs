use super::*;

pub(super) fn collect_websocket(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "ws") || has_import_or_require(trimmed, "websocket") {
            let metadata = metadata(
                input,
                symbols,
                line_number,
                "websocket",
                "Connection",
                "bidirectional",
                "WebSocketImport",
            );
            output.push(realtime_symbol(
                input,
                line_number,
                "WebSocket import",
                metadata,
            ));
        }
        if trimmed.contains("new WebSocket(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "websocket",
                "Connection",
                "bidirectional",
                "BrowserWebSocketConstructor",
            );
            metadata.endpoint = call_literal_argument(trimmed, "new WebSocket(");
            metadata.confidence = 9_500;
            output.push(realtime_symbol(
                input,
                line_number,
                "WebSocket connection",
                metadata,
            ));
        }
        if trimmed.contains("new WebSocket.Server(") || trimmed.contains(".Server({") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "websocket",
                "Connection",
                "inbound",
                "NodeWsServer",
            );
            metadata.confidence = 8_500;
            output.push(realtime_symbol(
                input,
                line_number,
                "WebSocket server",
                metadata,
            ));
        }
        if websocket_context(&input.source)
            && (trimmed.contains(".onmessage")
                || trimmed.contains(".addEventListener(\"message\"")
                || trimmed.contains(".addEventListener('message'")
                || trimmed.contains(".on(\"message\"")
                || trimmed.contains(".on('message'"))
        {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "websocket",
                "Listener",
                "inbound",
                "WebSocketMessageHandler",
            );
            metadata.event_name = Some("message".to_string());
            metadata.confidence = 9_000;
            output.push(realtime_symbol(
                input,
                line_number,
                "WebSocket message listener",
                metadata,
            ));
        }
        if trimmed.contains(".send(") && websocket_context(&input.source) {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "websocket",
                "Emitter",
                "outbound",
                "WebSocketSend",
            );
            metadata.confidence = 8_500;
            output.push(realtime_symbol(
                input,
                line_number,
                "WebSocket send",
                metadata,
            ));
        }
    }
    output
}

fn websocket_context(source: &str) -> bool {
    source.contains("new WebSocket(")
        || source.contains("WebSocket.Server")
        || source.contains("require(\"ws\")")
        || source.contains("require('ws')")
        || source.contains("from \"ws\"")
        || source.contains("from 'ws'")
}
