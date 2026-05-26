use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input, "json")];
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&input.source) {
        collect_value(&input, &value, String::new(), &mut symbols);
    }
    let file_name = input
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if file_name == "package.json" {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&input.source) {
            if let Some(name) = value.get("name").and_then(serde_json::Value::as_str) {
                symbols.push(package_symbol(
                    &input,
                    "json",
                    name,
                    1,
                    format!(
                        "package.manager=npm;package.name={name};package.file={}",
                        normalized_file(&input)
                    ),
                ));
            }
            for section in ["dependencies", "devDependencies", "peerDependencies"] {
                if let Some(object) = value.get(section).and_then(serde_json::Value::as_object) {
                    for dependency in object.keys() {
                        symbols.push(package_symbol(
                            &input,
                            "json",
                            dependency,
                            1,
                            format!("package.manager=npm;package.dependency={dependency};package.section={section};package.file={}", normalized_file(&input)),
                        ));
                    }
                }
            }
        }
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("json".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

fn collect_value(
    input: &ParseInput,
    value: &serde_json::Value,
    prefix: String,
    symbols: &mut Vec<ExtractedSymbol>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, child) in object {
        let key_path = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        };
        let redacted = is_sensitive_key(&key_path);
        symbols.push(config_symbol(
            input,
            "json",
            key_path.clone(),
            1,
            format!(
                "config.language=json;config.key_path={key_path};config.value_present={};config.value_redacted={redacted};config.file={}",
                child.is_string() && !redacted,
                normalized_file(input)
            ),
        ));
        collect_value(input, child, key_path, symbols);
    }
}
