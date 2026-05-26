use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input, "xml")];
    let mut stack: Vec<String> = Vec::new();
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let mut rest = line;
        while let Some(start) = rest.find('<') {
            rest = &rest[start + 1..];
            if rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('!') {
                if let Some(end) = rest.find('>') {
                    if rest.starts_with('/') {
                        stack.pop();
                    }
                    rest = &rest[end + 1..];
                    continue;
                }
                break;
            }
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = &rest[..end];
            let name = tag
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches('/')
                .to_string();
            if !name.is_empty() {
                stack.push(name.clone());
                let path = stack.join(".");
                symbols.push(config_symbol(
                    &input,
                    "xml",
                    path.clone(),
                    line_number,
                    format!(
                        "config.language=xml;config.element_path={path};config.file={}",
                        normalized_file(&input)
                    ),
                ));
                for attr in attribute_names(tag) {
                    let attr_path = format!("{path}@{attr}");
                    let value = attribute_value(tag, &attr).unwrap_or_default();
                    symbols.push(config_symbol(
                        &input,
                        "xml",
                        attr_path.clone(),
                        line_number,
                        config_metadata(
                            "xml",
                            &attr_path,
                            &value,
                            is_sensitive_key(&attr_path),
                            &normalized_file(&input),
                        ),
                    ));
                }
                if tag.ends_with('/') {
                    stack.pop();
                }
            }
            rest = &rest[end + 1..];
        }
    }
    collect_pom_dependencies(&input, &mut symbols);
    collect_xml_config_entries(&input, &mut symbols);
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("xml".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

fn attribute_names(tag: &str) -> Vec<String> {
    tag.split_whitespace()
        .skip(1)
        .filter_map(|part| part.split_once('=').map(|(name, _)| name.to_string()))
        .collect()
}

fn attribute_value(tag: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        let start = tag.find(&needle)? + needle.len();
        let rest = &tag[start..];
        let end = rest.find(quote)?;
        return Some(rest[..end].to_string());
    }
    None
}

fn collect_pom_dependencies(input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    let file_name = input
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if file_name != "pom.xml" {
        return;
    }
    let mut current_group: Option<String> = None;
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if let Some(value) = text_between(trimmed, "groupId") {
            current_group = Some(value);
        }
        if let Some(artifact) = text_between(trimmed, "artifactId") {
            let name = current_group
                .as_ref()
                .map(|group| format!("{group}:{artifact}"))
                .unwrap_or(artifact);
            symbols.push(package_symbol(
                input,
                "xml",
                name.clone(),
                line_number,
                format!(
                    "package.manager=maven;package.dependency={name};package.file={}",
                    normalized_file(input)
                ),
            ));
        }
    }
}

fn text_between(line: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = line.find(&start_tag)? + start_tag.len();
    let rest = &line[start..];
    let end = rest.find(&end_tag)?;
    Some(rest[..end].trim().to_string())
}

fn collect_xml_config_entries(input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.contains("<add ") {
            let key = attribute_value(trimmed, "key")
                .or_else(|| attribute_value(trimmed, "name"))
                .unwrap_or_else(|| "add".to_string());
            let value = attribute_value(trimmed, "value")
                .or_else(|| attribute_value(trimmed, "connectionString"))
                .unwrap_or_default();
            symbols.push(config_symbol(
                input,
                "xml",
                key.clone(),
                line_number,
                config_metadata(
                    "xml",
                    &key,
                    &value,
                    is_sensitive_key(&key),
                    &normalized_file(input),
                ),
            ));
        }
        if trimmed.contains("<bean ") {
            if let Some(id) =
                attribute_value(trimmed, "id").or_else(|| attribute_value(trimmed, "name"))
            {
                symbols.push(config_symbol(
                    input,
                    "xml",
                    id.clone(),
                    line_number,
                    format!(
                        "config.language=xml;config.spring_bean={id};config.file={}",
                        normalized_file(input)
                    ),
                ));
            }
        }
        if trimmed.contains("<manifest ") {
            if let Some(package) = attribute_value(trimmed, "package") {
                symbols.push(package_symbol(
                    input,
                    "xml",
                    package.clone(),
                    line_number,
                    format!(
                        "package.kind=android_manifest;package.name={package};package.file={}",
                        normalized_file(input)
                    ),
                ));
            }
        }
    }
}
