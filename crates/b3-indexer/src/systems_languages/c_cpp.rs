use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let language = language_from_path(&input.path).unwrap_or_else(|| "c".to_string());
    let mut symbols = vec![module_symbol(&input, &language)];

    if matches!(language.as_str(), "cmake" | "makefile" | "compile_commands") {
        symbols.push(symbol(
            &input,
            &language,
            input
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("build-metadata"),
            NodeKind::Package,
            1,
            format!("{language}.project=true;{language}.support=DetectOnly"),
        ));
        return Ok(ParsedFile {
            file_id: input.file_id,
            language: Some(language),
            symbols,
            relationships: Vec::new(),
        });
    }

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(include) = include_path(trimmed) {
            symbols.push(package_symbol(
                &input,
                &language,
                include.clone(),
                line_number,
                format!("{language}.include=true;{language}.include_path={include}"),
            ));
        }
        if let Some(name) = trimmed.strip_prefix("#define ").and_then(first_ident) {
            symbols.push(symbol(
                &input,
                &language,
                name,
                NodeKind::Variable,
                line_number,
                format!("{language}.macro=true;{language}.name={name}"),
            ));
        }
        for (prefix, kind, meta_key) in [
            ("struct ", NodeKind::Struct, "struct"),
            ("enum ", NodeKind::Enum, "enum"),
            ("typedef ", NodeKind::Variable, "typedef"),
            ("class ", NodeKind::Class, "class"),
            ("namespace ", NodeKind::Namespace, "namespace"),
        ] {
            if let Some(name) = declaration_name(trimmed, prefix) {
                symbols.push(symbol(
                    &input,
                    &language,
                    name.clone(),
                    kind,
                    line_number,
                    format!("{language}.{meta_key}=true;{language}.name={name}"),
                ));
            }
        }
        if let Some((name, kind)) = function_name(trimmed) {
            symbols.push(symbol(
                &input,
                &language,
                name.clone(),
                kind,
                line_number,
                format!("{language}.function=true;{language}.name={name}"),
            ));
        }
    }

    let relationships = relationships(&symbols);
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some(language),
        symbols,
        relationships,
    })
}

fn include_path(line: &str) -> Option<String> {
    let rest = line.strip_prefix("#include")?.trim();
    if let Some(value) = rest.strip_prefix('<') {
        return value.split('>').next().map(str::to_string);
    }
    literal_in(rest)
}

fn first_ident(value: &str) -> Option<&str> {
    value
        .trim()
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|name| !name.is_empty())
}

fn declaration_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    first_ident(rest).map(str::to_string)
}

fn function_name(line: &str) -> Option<(String, NodeKind)> {
    if line.starts_with('#')
        || !line.contains('(')
        || line.ends_with(';')
        || line.contains(" if ")
        || line.starts_with("if ")
        || line.starts_with("for ")
        || line.starts_with("while ")
        || line.starts_with("switch ")
    {
        return None;
    }
    let before = line.split_once('(')?.0.trim();
    let name = before
        .rsplit(|ch: char| !(ch == '_' || ch == ':' || ch.is_ascii_alphanumeric()))
        .find(|value| !value.is_empty())?;
    if name.is_empty() || ["return", "sizeof"].contains(&name) {
        return None;
    }
    let kind = if name.contains("::") {
        NodeKind::Method
    } else {
        NodeKind::Function
    };
    Some((name.rsplit("::").next().unwrap_or(name).to_string(), kind))
}
