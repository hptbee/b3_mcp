use super::*;

pub(super) fn collect_web_kafka(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !web_kafka_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if has_import_or_require(trimmed, "kafkajs")
            || has_import_or_require(trimmed, "kafka-node")
            || has_import_or_require(trimmed, "node-rdkafka")
        {
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka import",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "kafka",
                    "Producer",
                    "unknown",
                    "KafkaImport",
                ),
            ));
        }
        if trimmed.contains(".send(") && trimmed.contains("topic") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Producer",
                "outbound",
                "KafkaProducerSend",
            );
            metadata.topic = object_literal_string(trimmed, "topic");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(input, line_number, "Kafka send", metadata));
        }
        if trimmed.contains(".subscribe(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Consumer",
                "inbound",
                "KafkaConsumerSubscribe",
            );
            metadata.topic = object_literal_string(trimmed, "topic")
                .or_else(|| call_literal_argument(trimmed, ".subscribe("));
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka subscribe",
                metadata,
            ));
        }
        if trimmed.contains(".run(")
            && (trimmed.contains("eachMessage") || trimmed.contains("eachBatch"))
        {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Consumer",
                "inbound",
                "KafkaConsumerRun",
            );
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka consumer run",
                metadata,
            ));
        }
        if let Some(group) = object_literal_string(trimmed, "groupId") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Consumer",
                "inbound",
                "KafkaConsumerGroup",
            );
            metadata.consumer_group = Some(group);
            metadata.confidence = 7_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka consumer group",
                metadata,
            ));
        }
    }
    output
}

pub(super) fn collect_csharp_kafka(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    if !csharp_kafka_context(&input.source) {
        return Vec::new();
    }
    let mut output = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "using Confluent.Kafka;" {
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka using",
                metadata(
                    input,
                    symbols,
                    line_number,
                    "kafka",
                    "Producer",
                    "unknown",
                    "KafkaUsing",
                ),
            ));
        }
        if trimmed.contains(".ProduceAsync(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Producer",
                "outbound",
                "KafkaProduceAsync",
            );
            metadata.topic = call_literal_argument(trimmed, ".ProduceAsync(");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka produce",
                metadata,
            ));
        }
        if trimmed.contains(".Subscribe(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Consumer",
                "inbound",
                "KafkaSubscribe",
            );
            metadata.topic = call_literal_argument(trimmed, ".Subscribe(");
            metadata.confidence = 9_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka subscribe",
                metadata,
            ));
        }
        if trimmed.contains(".Consume(") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Consumer",
                "inbound",
                "KafkaConsume",
            );
            metadata.confidence = 8_000;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka consume",
                metadata,
            ));
        }
        if let Some(group) = named_literal(trimmed, "GroupId") {
            let mut metadata = metadata(
                input,
                symbols,
                line_number,
                "kafka",
                "Consumer",
                "inbound",
                "KafkaConsumerGroup",
            );
            metadata.consumer_group = Some(group);
            metadata.confidence = 7_500;
            output.push(messaging_symbol(
                input,
                line_number,
                "Kafka consumer group",
                metadata,
            ));
        }
    }
    output
}

fn web_kafka_context(source: &str) -> bool {
    source.contains("kafkajs") || source.contains("kafka-node") || source.contains("node-rdkafka")
}

fn csharp_kafka_context(source: &str) -> bool {
    source.contains("Confluent.Kafka")
        || source.contains("ProduceAsync")
        || source.contains("Subscribe(")
}
