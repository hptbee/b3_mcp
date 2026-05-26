use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let file_name = input
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let safe_example = is_safe_env_file_name(&file_name);
    let mut symbols = vec![module_symbol(&input, "env")];
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_start_matches("export ").trim();
        if key.is_empty() {
            continue;
        }
        let value = clean_scalar(value);
        let force_redacted = !safe_example;
        let mut metadata =
            config_metadata("env", key, &value, force_redacted, &normalized_file(&input));
        metadata.push_str(&format!(
            ";config.env_file_safe={safe_example};config.resolution=definition;config.confidence={}",
            if safe_example { 8500 } else { 5000 }
        ));
        symbols.push(config_symbol(&input, "env", key, line_number, metadata));
        symbols.extend(env_reference_symbols(
            &input,
            "env",
            key,
            &value,
            line_number,
        ));
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("env".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

fn is_safe_env_file_name(file_name: &str) -> bool {
    matches!(
        file_name,
        ".env.example"
            | ".env.sample"
            | ".env.defaults"
            | ".env.template"
            | "example.env"
            | "sample.env"
    )
}
