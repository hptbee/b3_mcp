use super::amqp::first_string_args;
use super::*;

pub(super) fn collect_csharp_rabbitmq(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !rabbitmq_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "using RabbitMQ.Client;" {
            output.push(messaging_symbol(
                input,
                line_number,
                "RabbitMQ using",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "rabbitmq",
                    "Producer",
                    "unknown",
                    "RabbitMqUsing",
                ),
            ));
        }
        if trimmed.contains(".BasicPublish(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "rabbitmq",
                "Publisher",
                "outbound",
                "RabbitMqPublish",
            );
            metadata.exchange = named_literal(trimmed, "exchange:");
            metadata.routing_key = named_literal(trimmed, "routingKey:");
            if metadata.exchange.is_none() || metadata.routing_key.is_none() {
                let args = first_string_args(trimmed);
                metadata.exchange = metadata.exchange.or_else(|| args.first().cloned());
                metadata.routing_key = metadata.routing_key.or_else(|| args.get(1).cloned());
            }
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "RabbitMQ publish",
                metadata,
            ));
        }
        if trimmed.contains(".BasicConsume(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "rabbitmq",
                "Consumer",
                "inbound",
                "RabbitMqConsume",
            );
            metadata.queue = named_literal(trimmed, "queue:")
                .or_else(|| first_string_args(trimmed).first().cloned());
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "RabbitMQ consume",
                metadata,
            ));
        }
        if trimmed.contains(".QueueDeclare(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "rabbitmq",
                "Queue",
                "unknown",
                "RabbitMqQueueDeclare",
            );
            metadata.queue = named_literal(trimmed, "queue:")
                .or_else(|| first_string_args(trimmed).first().cloned());
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "RabbitMQ queue",
                metadata,
            ));
        }
        if trimmed.contains(".ExchangeDeclare(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "rabbitmq",
                "Exchange",
                "unknown",
                "RabbitMqExchangeDeclare",
            );
            metadata.exchange = named_literal(trimmed, "exchange:")
                .or_else(|| first_string_args(trimmed).first().cloned());
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "RabbitMQ exchange",
                metadata,
            ));
        }
        if trimmed.contains(".QueueBind(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "rabbitmq",
                "RoutingKey",
                "unknown",
                "RabbitMqQueueBind",
            );
            metadata.queue = named_literal(trimmed, "queue:");
            metadata.exchange = named_literal(trimmed, "exchange:");
            metadata.routing_key = named_literal(trimmed, "routingKey:");
            let args = first_string_args(trimmed);
            metadata.queue = metadata.queue.or_else(|| args.first().cloned());
            metadata.exchange = metadata.exchange.or_else(|| args.get(1).cloned());
            metadata.routing_key = metadata.routing_key.or_else(|| args.get(2).cloned());
            metadata.confidence = 8_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "RabbitMQ bind",
                metadata,
            ));
        }
    }
    output
}

fn rabbitmq_context(source: &str) -> bool {
    source.contains("RabbitMQ.Client")
        || source.contains("BasicPublish")
        || source.contains("BasicConsume")
}
