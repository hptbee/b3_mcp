use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input, "yaml")];
    let file_name = input
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut path_stack: Vec<(usize, String)> = Vec::new();

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
            continue;
        }
        let indent = line.chars().take_while(|ch| *ch == ' ').count();
        while path_stack.last().is_some_and(|(level, _)| *level >= indent) {
            path_stack.pop();
        }
        if let Some((raw_key, raw_value)) = trimmed.split_once(':') {
            let key = raw_key.trim().trim_start_matches("- ").trim();
            if key.is_empty() {
                continue;
            }
            path_stack.push((indent, key.to_string()));
            let key_path = path_stack
                .iter()
                .map(|(_, part)| part.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let value = clean_scalar(raw_value);
            let redacted = is_sensitive_key(&key_path) || file_name.contains("secret");
            let mut metadata = config_metadata(
                "yaml",
                &key_path,
                &value,
                redacted,
                &normalized_file(&input),
            );
            if key_path.ends_with("kind") && value == "Secret" {
                metadata.push_str(";config.kubernetes_secret=true");
            }
            if key_path.contains("configMapRef") || key_path.contains("configMapKeyRef") {
                metadata.push_str(";config.reference_kind=configmap");
            }
            symbols.push(config_symbol(
                &input,
                "yaml",
                key_path.clone(),
                line_number,
                metadata,
            ));
            symbols.extend(env_reference_symbols(
                &input,
                "yaml",
                &key_path,
                &value,
                line_number,
            ));
        }
    }

    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("yaml".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}
