use super::*;

pub(crate) fn is_ksqldb_file(path: &Path, source: &str) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.ends_with(".ksql")
        || name.ends_with(".ksql.sql")
        || source.to_ascii_uppercase().contains("CREATE STREAM")
        || source.to_ascii_uppercase().contains("CREATE TABLE")
        || source
            .to_ascii_uppercase()
            .contains("CREATE SOURCE CONNECTOR")
        || source
            .to_ascii_uppercase()
            .contains("CREATE SINK CONNECTOR")
}

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input)];
    for (index, statement) in input.source.split(';').enumerate() {
        let line = line_for_statement(&input.source, statement).unwrap_or(index + 1);
        let upper = statement.to_ascii_uppercase();
        if let Some((kind, name)) = create_name(&upper, statement) {
            symbols.push(config_symbol(
                &input,
                &name,
                line,
                format!(
                    "ksqldb.kind={kind};ksqldb.name={name};ksqldb.file={}",
                    normalized_file(&input)
                ),
            ));
            if let Some(topic) = topic_name(statement) {
                symbols.push(messaging_symbol(
                    &input,
                    line,
                    &format!("Kafka topic {topic}"),
                    MessagingMetadata {
                        technology: "kafka".to_string(),
                        kind: "Topic".to_string(),
                        direction: if kind == "Stream" {
                            "consumer"
                        } else {
                            "producer"
                        }
                        .to_string(),
                        topic: Some(topic),
                        queue: None,
                        exchange: None,
                        routing_key: None,
                        pattern: None,
                        consumer_group: None,
                        file_path: normalized_file(&input),
                        symbol_id: None,
                        class_name: None,
                        function_name: None,
                        method_name: None,
                        line_start: line,
                        line_end: line,
                        confidence: 8_000,
                        source_kind: "KsqlDbTopicLiteral".to_string(),
                    },
                ));
            }
        }
        if upper.contains("INSERT INTO") {
            if let Some(target) = word_after(&upper, statement, "INSERT INTO") {
                symbols.push(config_symbol(
                    &input,
                    &target,
                    line,
                    format!(
                        "ksqldb.insert_into={target};ksqldb.file={}",
                        normalized_file(&input)
                    ),
                ));
            }
        }
        if upper.contains(" FROM ") {
            if let Some(source) = word_after(&upper, statement, " FROM ") {
                symbols.push(config_symbol(
                    &input,
                    &source,
                    line,
                    format!(
                        "ksqldb.depends_on={source};ksqldb.emit_changes={};ksqldb.file={}",
                        upper.contains("EMIT CHANGES"),
                        normalized_file(&input)
                    ),
                ));
            }
        }
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("ksql".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:ksql-module", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("ksqldb")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line: input.source.lines().count().max(1),
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some("ksqldb.file=true".to_string()),
    }
}

fn config_symbol(input: &ParseInput, name: &str, line: usize, metadata: String) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:ksqldb:{name}:{line}", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name: name.to_string(),
        kind: NodeKind::ConfigKey,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: line,
        end_column: 0,
        visibility: Some(metadata),
    }
}

fn messaging_symbol(
    input: &ParseInput,
    line: usize,
    name: &str,
    metadata: MessagingMetadata,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:ksqldb-messaging:{name}:{line}", input.file_id.as_str()),
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

fn create_name(upper: &str, original: &str) -> Option<(&'static str, String)> {
    for (needle, kind) in [
        ("CREATE STREAM", "Stream"),
        ("CREATE TABLE", "Table"),
        ("CREATE SOURCE CONNECTOR", "SourceConnector"),
        ("CREATE SINK CONNECTOR", "SinkConnector"),
    ] {
        if upper.contains(needle) {
            return word_after(upper, original, needle).map(|name| (kind, name));
        }
    }
    None
}

fn word_after(upper: &str, original: &str, marker: &str) -> Option<String> {
    let index = upper.find(marker)? + marker.len();
    original[index..]
        .trim()
        .trim_start_matches("IF NOT EXISTS")
        .trim()
        .split(|ch: char| !(ch == '_' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_matches('`').to_string())
}

fn topic_name(statement: &str) -> Option<String> {
    for key in ["KAFKA_TOPIC", "kafka_topic"] {
        if let Some(index) = statement.find(key) {
            let rest = &statement[index + key.len()..];
            if let Some(value) = literal_in(rest) {
                return Some(value);
            }
        }
    }
    None
}

fn literal_in(value: &str) -> Option<String> {
    let start = value.find(['"', '\''])?;
    let quote = value.as_bytes().get(start).copied()? as char;
    let rest = &value[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn line_for_statement(source: &str, statement: &str) -> Option<usize> {
    let offset = source.find(statement.trim())?;
    Some(source[..offset].lines().count().max(1))
}

fn encode_messaging_metadata(metadata: &MessagingMetadata) -> String {
    [
        ("messaging.technology", Some(metadata.technology.as_str())),
        ("messaging.kind", Some(metadata.kind.as_str())),
        ("messaging.direction", Some(metadata.direction.as_str())),
        ("messaging.topic", metadata.topic.as_deref()),
        ("messaging.file", Some(metadata.file_path.as_str())),
        ("messaging.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", value.replace(';', "%3B"))))
    .chain([
        format!("messaging.line_start={}", metadata.line_start),
        format!("messaging.line_end={}", metadata.line_end),
        format!("messaging.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}
