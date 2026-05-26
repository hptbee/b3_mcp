use super::*;

mod env;
mod json;
mod redaction;
mod toml;
mod xml;
mod yaml;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    match language_from_path(&input.path).as_deref() {
        Some("yaml") => yaml::parse(input),
        Some("json") => json::parse(input),
        Some("toml") => toml::parse(input),
        Some("xml") => xml::parse(input),
        Some("env") => env::parse(input),
        _ => NoopTreeSitterParser.parse(input),
    }
}

pub(crate) fn module_symbol(input: &ParseInput, language: &str) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}-config:{}",
                input.file_id.as_str(),
                input.path.display()
            ),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("config")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line: input.source.lines().count().max(1),
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some(format!("{language}.file=true;config.file=true")),
    }
}

pub(crate) fn config_symbol(
    input: &ParseInput,
    language: &str,
    name: impl Into<String>,
    line: usize,
    metadata: String,
) -> ExtractedSymbol {
    let name = name.into();
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:{language}:config:{name}:{line}", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name,
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

pub(crate) fn package_symbol(
    input: &ParseInput,
    language: &str,
    name: impl Into<String>,
    line: usize,
    metadata: String,
) -> ExtractedSymbol {
    let name = name.into();
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}:package:{name}:{line}",
                input.file_id.as_str()
            ),
        )),
        file_id: input.file_id.clone(),
        name,
        kind: NodeKind::Package,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: line,
        end_column: 0,
        visibility: Some(metadata),
    }
}

pub(crate) fn is_sensitive_key(key: &str) -> bool {
    redaction::is_sensitive_key(key)
}

pub(crate) fn value_class(key: &str, value: &str, force_redacted: bool) -> &'static str {
    redaction::value_class(key, value, force_redacted)
}

pub(crate) fn safe_value_hint(key: &str, value: &str, force_redacted: bool) -> Option<String> {
    redaction::safe_value_hint(key, value, force_redacted)
}

pub(crate) fn config_metadata(
    language: &str,
    key_name: &str,
    value: &str,
    force_redacted: bool,
    file: &str,
) -> String {
    let class = value_class(key_name, value, force_redacted);
    let mut parts = vec![
        format!("config.language={language}"),
        format!("config.key_path={key_name}"),
        format!(
            "config.value_present={}",
            !value.trim().is_empty() && class != "secret_like"
        ),
        format!("config.value_class={class}"),
        format!("config.value_redacted={}", class == "secret_like"),
        format!("config.file={file}"),
    ];
    if let Some(hint) = safe_value_hint(key_name, value, force_redacted) {
        parts.push(format!(
            "config.safe_value_hint={}",
            hint.replace(';', "%3B")
        ));
    }
    parts.join(";")
}

pub(crate) fn env_reference_symbols(
    input: &ParseInput,
    language: &str,
    owner: &str,
    value: &str,
    line: usize,
) -> Vec<ExtractedSymbol> {
    redaction::env_refs(value)
        .into_iter()
        .map(|name| {
            config_symbol(
                input,
                language,
                format!("{owner}->{name}"),
                line,
                format!(
                    "config.language={language};config.reference={name};config.reference_owner={owner};config.reference_kind=env;config.resolution=unresolved;config.confidence=5000;config.file={}",
                    normalized_file(input)
                ),
            )
        })
        .collect()
}

pub(crate) fn clean_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches(',')
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

pub(crate) fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}
