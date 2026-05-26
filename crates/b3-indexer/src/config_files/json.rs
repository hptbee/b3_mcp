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
    } else if file_name == "launchsettings.json" {
        collect_launch_settings(&input, &mut symbols);
    } else if file_name.ends_with("appsettings.json") || file_name.starts_with("appsettings.") {
        collect_appsettings_refs(&input, &mut symbols);
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
        let value = child.as_str().unwrap_or_default();
        let redacted = is_sensitive_key(&key_path);
        symbols.push(config_symbol(
            input,
            "json",
            key_path.clone(),
            1,
            config_metadata("json", &key_path, value, redacted, &normalized_file(input)),
        ));
        symbols.extend(env_reference_symbols(input, "json", &key_path, value, 1));
        collect_value(input, child, key_path, symbols);
    }
}

fn collect_launch_settings(input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&input.source) else {
        return;
    };
    let Some(profiles) = value.get("profiles").and_then(serde_json::Value::as_object) else {
        return;
    };
    for (profile, profile_value) in profiles {
        if let Some(urls) = profile_value
            .get("applicationUrl")
            .and_then(serde_json::Value::as_str)
        {
            for url in urls.split(';').filter(|url| !url.trim().is_empty()) {
                symbols.push(config_symbol(
                    input,
                    "json",
                    format!("launchSettings.{profile}.applicationUrl"),
                    1,
                    format!(
                        "config.language=json;config.key_path=launchSettings.{profile}.applicationUrl;config.value_class=url;config.safe_value_hint={};config.file={}",
                        url.replace(';', "%3B"),
                        normalized_file(input)
                    ),
                ));
            }
        }
    }
}

fn collect_appsettings_refs(input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    if !input.source.contains("RabbitMq") && !input.source.contains("Kafka") {
        return;
    }
    symbols.push(config_symbol(
        input,
        "json",
        "appsettings.messaging",
        1,
        format!(
            "config.language=json;config.reference_kind=messaging_config;config.resolution=definition;config.confidence=8000;config.file={}",
            normalized_file(input)
        ),
    ));
}
