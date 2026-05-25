use super::*;

mod compose;
mod docker;
mod kubernetes;
mod terraform;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let language = infrastructure_language(&input.path, &input.source);
    let mut symbols = Vec::new();
    symbols.extend(docker::collect_dockerfile(&input));
    symbols.extend(compose::collect_compose(&input));
    symbols.extend(kubernetes::collect_kubernetes(&input));
    symbols.extend(terraform::collect_terraform(&input));

    Ok(ParsedFile {
        file_id: input.file_id,
        language,
        symbols,
        relationships: Vec::new(),
    })
}

pub(crate) fn is_infrastructure_file(path: &Path, source: &str) -> bool {
    docker::is_dockerfile(path)
        || compose::is_compose_file(path)
        || terraform::is_terraform_file(path)
        || kubernetes::is_kubernetes_yaml(path, source)
}

fn infrastructure_language(path: &Path, source: &str) -> Option<String> {
    if docker::is_dockerfile(path) {
        Some("dockerfile".to_string())
    } else if compose::is_compose_file(path) {
        Some("docker_compose".to_string())
    } else if terraform::is_terraform_file(path) {
        Some("terraform".to_string())
    } else if kubernetes::is_kubernetes_yaml(path, source) {
        Some("kubernetes".to_string())
    } else {
        language_from_path(path)
    }
}

pub(crate) fn infrastructure_symbol(
    input: &ParseInput,
    line: usize,
    name: &str,
    metadata: InfrastructureMetadata,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:infrastructure:{}:{}:{}:{}",
                input.file_id.as_str(),
                metadata.technology,
                metadata.source_kind,
                name,
                line
            ),
        )),
        file_id: input.file_id.clone(),
        name: name.to_string(),
        kind: NodeKind::ConfigKey,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: metadata.line_end,
        end_column: 0,
        visibility: Some(encode_infrastructure_metadata(&metadata)),
    }
}

pub(crate) fn metadata(
    input: &ParseInput,
    line_start: usize,
    line_end: usize,
    technology: &str,
    kind: &str,
    source_kind: &str,
) -> InfrastructureMetadata {
    InfrastructureMetadata {
        technology: technology.to_string(),
        kind: kind.to_string(),
        name: None,
        resource_type: None,
        provider: None,
        image: None,
        service_name: None,
        container_name: None,
        namespace: None,
        ports: Vec::new(),
        env_keys: Vec::new(),
        labels: Vec::new(),
        selectors: Vec::new(),
        file_path: normalized_file(input),
        symbol_id: None,
        line_start,
        line_end,
        confidence: 8_000,
        source_kind: source_kind.to_string(),
    }
}

fn encode_infrastructure_metadata(metadata: &InfrastructureMetadata) -> String {
    [
        (
            "infrastructure.technology",
            Some(metadata.technology.as_str()),
        ),
        ("infrastructure.kind", Some(metadata.kind.as_str())),
        ("infrastructure.name", metadata.name.as_deref()),
        (
            "infrastructure.resource_type",
            metadata.resource_type.as_deref(),
        ),
        ("infrastructure.provider", metadata.provider.as_deref()),
        ("infrastructure.image", metadata.image.as_deref()),
        (
            "infrastructure.service_name",
            metadata.service_name.as_deref(),
        ),
        (
            "infrastructure.container_name",
            metadata.container_name.as_deref(),
        ),
        ("infrastructure.namespace", metadata.namespace.as_deref()),
        ("infrastructure.file", Some(metadata.file_path.as_str())),
        ("infrastructure.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("infrastructure.ports={}", metadata.ports.join(",")),
        format!("infrastructure.env_keys={}", metadata.env_keys.join(",")),
        format!("infrastructure.labels={}", metadata.labels.join(",")),
        format!("infrastructure.selectors={}", metadata.selectors.join(",")),
        format!("infrastructure.line_start={}", metadata.line_start),
        format!("infrastructure.line_end={}", metadata.line_end),
        format!("infrastructure.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

pub(crate) fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches(',')
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

pub(crate) fn value_after_colon(line: &str) -> Option<String> {
    line.split_once(':')
        .map(|(_, value)| clean_value(value))
        .filter(|value| !value.is_empty())
}

pub(crate) fn list_value(line: &str) -> Option<String> {
    line.trim()
        .strip_prefix("- ")
        .map(clean_value)
        .filter(|value| !value.is_empty())
}

pub(crate) fn env_key(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches("- ").trim();
    let key = value
        .split_once('=')
        .map(|(key, _)| key)
        .or_else(|| value.split_once(':').map(|(key, _)| key))
        .unwrap_or(value)
        .trim();
    if key.is_empty() {
        None
    } else {
        Some(clean_value(key))
    }
}

pub(crate) fn leading_spaces(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

pub(crate) fn scalar_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    trimmed
        .strip_suffix(':')
        .map(clean_value)
        .filter(|value| !value.is_empty())
}

fn escape_metadata(value: &str) -> String {
    value.replace(';', "%3B").replace('\n', "\\n")
}

#[cfg(test)]
fn unescape_metadata(value: &str) -> String {
    value.replace("%3B", ";").replace("\\n", "\n")
}

#[cfg(test)]
pub(crate) fn infrastructure_metadata_value(metadata: &str, key: &str) -> Option<String> {
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&format!("infrastructure.{key}="))
            .map(unescape_metadata)
    })
}
