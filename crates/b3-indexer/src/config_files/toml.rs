use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input, "toml")];
    let mut table = String::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            table = trimmed.trim_matches(['[', ']']).to_string();
            symbols.push(config_symbol(
                &input,
                "toml",
                table.clone(),
                line_number,
                format!(
                    "config.language=toml;config.table={table};config.file={}",
                    normalized_file(&input)
                ),
            ));
            continue;
        }
        if let Some((key, raw_value)) = trimmed.split_once('=') {
            let key = key.trim();
            let key_path = if table.is_empty() {
                key.to_string()
            } else {
                format!("{table}.{key}")
            };
            let redacted = is_sensitive_key(&key_path);
            symbols.push(config_symbol(
                &input,
                "toml",
                key_path.clone(),
                line_number,
                format!(
                    "config.language=toml;config.key_path={key_path};config.value_present={};config.value_redacted={redacted};config.file={}",
                    !clean_scalar(raw_value).is_empty() && !redacted,
                    normalized_file(&input)
                ),
            ));
            if table.contains("dependencies") {
                symbols.push(package_symbol(
                    &input,
                    "toml",
                    key,
                    line_number,
                    format!("package.manager=toml;package.dependency={key};package.section={table};package.file={}", normalized_file(&input)),
                ));
            }
        }
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("toml".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}
