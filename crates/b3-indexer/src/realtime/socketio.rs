use super::*;

pub(super) fn collect_socketio(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !socketio_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "socket.io")
            || has_import_or_require(trimmed, "socket.io-client")
        {
            let metadata = metadata(
                input,
                symbols,
                line_number,
                "socketio",
                "Connection",
                "bidirectional",
                "SocketIoImport",
            );
            output.push(realtime_symbol(
                input,
                line_number,
                "Socket.IO import",
                metadata,
            ));
        }
        if trimmed.contains("io(") || trimmed.contains("new Server(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "socketio",
                "Connection",
                "bidirectional",
                "SocketIoConnection",
            );
            metadata.endpoint = literal_string_argument(trimmed);
            metadata.confidence = 8_000;
            output.push(realtime_symbol(
                input,
                line_number,
                "Socket.IO connection",
                metadata,
            ));
        }
        if let Some(event) = socket_event(trimmed, ".on(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "socketio",
                "Listener",
                if event == "connection" {
                    "inbound"
                } else {
                    "bidirectional"
                },
                "SocketIoOn",
            );
            metadata.event_name = Some(event.clone());
            metadata.confidence = 9_000;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("Socket.IO on {event}"),
                metadata,
            ));
        }
        if let Some(event) = socket_event(trimmed, ".emit(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "socketio",
                "Emitter",
                "outbound",
                "SocketIoEmit",
            );
            metadata.event_name = Some(event.clone());
            metadata.confidence = 9_000;
            output.push(realtime_symbol(
                input,
                line_number,
                &format!("Socket.IO emit {event}"),
                metadata,
            ));
        }
    }
    output
}

fn socketio_context(source: &str) -> bool {
    source.contains("socket.io")
        || source.contains("socket.io-client")
        || source.contains("Server } from \"socket.io\"")
        || source.contains("Server } from 'socket.io'")
}

fn socket_event(line: &str, call: &str) -> Option<String> {
    call_literal_argument(line, call)
}
