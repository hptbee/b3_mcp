use super::*;

mod amqp;
mod kafka;
mod nestjs;
mod pubsub;
mod rabbitmq;

pub(crate) fn collect_web_messaging(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    output.extend(amqp::collect_amqp(input, symbols));
    output.extend(kafka::collect_web_kafka(input, symbols));
    output.extend(pubsub::collect_web_pubsub(input, symbols));
    output.extend(nestjs::collect_nestjs_messaging(input, symbols));
    output
}

pub(crate) fn collect_csharp_messaging(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    output.extend(rabbitmq::collect_csharp_rabbitmq(input, symbols));
    output.extend(kafka::collect_csharp_kafka(input, symbols));
    output.extend(pubsub::collect_csharp_pubsub(input, symbols));
    output
}

pub fn detect_package_json_messaging_technologies(
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
            let Some((id, name, support_level)) = package_messaging(package_name) else {
                continue;
            };
            if detected
                .iter()
                .any(|tech: &DetectedTechnology| tech.id == id)
            {
                continue;
            }
            detected.push(messaging_technology(
                id,
                name,
                support_level,
                &format!("package.json:{section}:{package_name}"),
            ));
        }
    }
    Ok(detected)
}

pub fn detect_csproj_messaging_technologies(
    source: &str,
) -> ContractResult<Vec<DetectedTechnology>> {
    let lower = source.to_ascii_lowercase();
    let mut detected = Vec::new();
    for (needle, id, name, support) in [
        (
            "rabbitmq.client",
            "rabbitmq",
            "RabbitMQ.Client",
            TechnologySupportLevel::Basic,
        ),
        (
            "confluent.kafka",
            "kafka",
            "Confluent.Kafka",
            TechnologySupportLevel::Basic,
        ),
        (
            "google.cloud.pubsub.v1",
            "google_pubsub",
            "Google Cloud Pub/Sub",
            TechnologySupportLevel::Basic,
        ),
        (
            "masstransit",
            "masstransit",
            "MassTransit",
            TechnologySupportLevel::DetectOnly,
        ),
    ] {
        if lower.contains(needle) {
            detected.push(messaging_technology(id, name, support, "csproj"));
        }
    }
    Ok(detected)
}

fn package_messaging(
    package_name: &str,
) -> Option<(&'static str, &'static str, TechnologySupportLevel)> {
    match package_name {
        "amqplib" | "amqp-connection-manager" => {
            Some(("amqp", "AMQP", TechnologySupportLevel::Basic))
        }
        "kafkajs" | "kafka-node" | "node-rdkafka" => {
            Some(("kafka", "Kafka", TechnologySupportLevel::Basic))
        }
        "@google-cloud/pubsub" => Some((
            "google_pubsub",
            "Google Cloud Pub/Sub",
            TechnologySupportLevel::Basic,
        )),
        "@nestjs/microservices" => Some((
            "nestjs_messaging",
            "NestJS Messaging",
            TechnologySupportLevel::Basic,
        )),
        "pubsub-js" => Some((
            "pubsub",
            "Generic Pub/Sub",
            TechnologySupportLevel::DetectOnly,
        )),
        "nats" => Some(("nats", "NATS", TechnologySupportLevel::DetectOnly)),
        _ => None,
    }
}

fn messaging_technology(
    id: &str,
    name: &str,
    support_level: TechnologySupportLevel,
    source: &str,
) -> DetectedTechnology {
    DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind: TechnologyKind::Messaging,
        support_level,
        capabilities: vec![
            TechnologyCapability::DetectPackage,
            TechnologyCapability::DetectImport,
            TechnologyCapability::ExtractMessaging,
        ],
        source: source.to_string(),
    }
}

pub(crate) fn messaging_symbol(
    input: &ParseInput,
    line: usize,
    name: &str,
    metadata: MessagingMetadata,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:messaging:{}:{}:{}:{}",
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
        visibility: Some(encode_messaging_metadata(&metadata)),
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
) -> MessagingMetadata {
    MessagingMetadata {
        technology: technology.to_string(),
        kind: kind.to_string(),
        direction: direction.to_string(),
        topic: None,
        queue: None,
        exchange: None,
        routing_key: None,
        pattern: None,
        consumer_group: None,
        file_path: normalized_file(input),
        symbol_id: containing_symbol_id(symbols, line),
        class_name: containing_class_name(symbols, line),
        function_name: containing_method_name(symbols, line),
        method_name: containing_method_name(symbols, line),
        line_start: line,
        line_end: line,
        confidence: 8_000,
        source_kind: source_kind.to_string(),
    }
}

fn encode_messaging_metadata(metadata: &MessagingMetadata) -> String {
    [
        ("messaging.technology", Some(metadata.technology.as_str())),
        ("messaging.kind", Some(metadata.kind.as_str())),
        ("messaging.direction", Some(metadata.direction.as_str())),
        ("messaging.topic", metadata.topic.as_deref()),
        ("messaging.queue", metadata.queue.as_deref()),
        ("messaging.exchange", metadata.exchange.as_deref()),
        ("messaging.routing_key", metadata.routing_key.as_deref()),
        ("messaging.pattern", metadata.pattern.as_deref()),
        (
            "messaging.consumer_group",
            metadata.consumer_group.as_deref(),
        ),
        ("messaging.file", Some(metadata.file_path.as_str())),
        ("messaging.class", metadata.class_name.as_deref()),
        ("messaging.function", metadata.function_name.as_deref()),
        ("messaging.method", metadata.method_name.as_deref()),
        ("messaging.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("messaging.line_start={}", metadata.line_start),
        format!("messaging.line_end={}", metadata.line_end),
        format!("messaging.confidence={}", metadata.confidence),
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

pub(crate) fn named_literal(line: &str, name: &str) -> Option<String> {
    let start = line.find(name)? + name.len();
    literal_string_argument(line.get(start..)?)
}

pub(crate) fn object_literal_string(line: &str, key: &str) -> Option<String> {
    for needle in [format!("{key}:"), format!("{key} =")] {
        if let Some(value) = named_literal(line, &needle) {
            return Some(value);
        }
    }
    None
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

#[cfg(test)]
pub(crate) fn messaging_metadata_value(metadata: &str, key: &str) -> Option<String> {
    prefixed_metadata_value(metadata, "messaging", key)
}
