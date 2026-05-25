use super::*;

pub(super) fn collect_amqp(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !amqp_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "amqplib")
            || has_import_or_require(trimmed, "amqp-connection-manager")
        {
            output.push(messaging_symbol(
                input,
                line_number,
                "AMQP import",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "amqp",
                    "Producer",
                    "unknown",
                    "AmqpImport",
                ),
            ));
        }
        if trimmed.contains(".publish(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "amqp",
                "Publisher",
                "outbound",
                "AmqpPublish",
            );
            let args = first_string_args(trimmed);
            metadata.exchange = args.first().cloned();
            metadata.routing_key = args.get(1).cloned();
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "AMQP publish",
                metadata,
            ));
        }
        if trimmed.contains(".sendToQueue(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "amqp",
                "Producer",
                "outbound",
                "AmqpSendToQueue",
            );
            metadata.queue = call_literal_argument(trimmed, ".sendToQueue(");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "AMQP sendToQueue",
                metadata,
            ));
        }
        if trimmed.contains(".consume(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "amqp",
                "Consumer",
                "inbound",
                "AmqpConsume",
            );
            metadata.queue = call_literal_argument(trimmed, ".consume(");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "AMQP consume",
                metadata,
            ));
        }
        if trimmed.contains(".assertExchange(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "amqp",
                "Exchange",
                "unknown",
                "AmqpAssertExchange",
            );
            metadata.exchange = call_literal_argument(trimmed, ".assertExchange(");
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "AMQP exchange",
                metadata,
            ));
        }
        if trimmed.contains(".assertQueue(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "amqp",
                "Queue",
                "unknown",
                "AmqpAssertQueue",
            );
            metadata.queue = call_literal_argument(trimmed, ".assertQueue(");
            metadata.confidence = 8_500;
            output.push(messaging_symbol(input, line_number, "AMQP queue", metadata));
        }
        if trimmed.contains(".bindQueue(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "amqp",
                "RoutingKey",
                "unknown",
                "AmqpBindQueue",
            );
            let args = first_string_args(trimmed);
            metadata.queue = args.first().cloned();
            metadata.exchange = args.get(1).cloned();
            metadata.routing_key = args.get(2).cloned();
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "AMQP bindQueue",
                metadata,
            ));
        }
    }
    output
}

fn amqp_context(source: &str) -> bool {
    source.contains("amqplib") || source.contains("amqp-connection-manager")
}

pub(crate) fn first_string_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut rest = line;
    while let Some(value) = literal_string_argument(rest) {
        let quote_pos = rest.find(['"', '\'']).unwrap_or(0);
        let quote = rest.as_bytes().get(quote_pos).copied().unwrap_or(b'"') as char;
        let after = &rest[quote_pos + 1..];
        let Some(end) = after.find(quote) else {
            break;
        };
        args.push(value);
        rest = &after[end + 1..];
    }
    args
}
