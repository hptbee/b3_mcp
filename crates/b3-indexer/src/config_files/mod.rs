use super::*;

mod json;
mod toml;
mod xml;
mod yaml;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    match language_from_path(&input.path).as_deref() {
        Some("yaml") => yaml::parse(input),
        Some("json") => json::parse(input),
        Some("toml") => toml::parse(input),
        Some("xml") => xml::parse(input),
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
    let key = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "apikey",
        "api_key",
        "connectionstring",
        "connection_string",
    ]
    .iter()
    .any(|needle| key.contains(needle))
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
