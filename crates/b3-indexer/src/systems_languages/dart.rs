use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let language = language_from_path(&input.path).unwrap_or_else(|| "dart".to_string());
    let mut symbols = vec![module_symbol(&input, &language)];
    if language == "dart_project" {
        symbols.push(symbol(
            &input,
            &language,
            "Dart project",
            NodeKind::Package,
            1,
            "dart.project=true;dart.support=Basic".to_string(),
        ));
    }
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("import ")
            || trimmed.starts_with("export ")
            || trimmed.starts_with("part ")
        {
            if let Some(path) = literal_in(trimmed) {
                symbols.push(package_symbol(
                    &input,
                    &language,
                    path.clone(),
                    line_number,
                    format!("dart.import=true;dart.import_path={path}"),
                ));
            }
        }
        for (prefix, kind, key) in [
            ("class ", NodeKind::Class, "class"),
            ("mixin ", NodeKind::Class, "mixin"),
            ("enum ", NodeKind::Enum, "enum"),
        ] {
            if let Some(name) = identifier_after(trimmed, prefix) {
                let mut metadata = format!("dart.{key}=true;dart.name={name}");
                if trimmed.contains("Widget") {
                    metadata.push_str(";flutter.widget=true");
                }
                symbols.push(symbol(&input, &language, name, kind, line_number, metadata));
            }
        }
        if let Some(name) = function_name(trimmed) {
            let metadata = if name == "build" {
                "dart.method=true;flutter.build_method=true".to_string()
            } else {
                format!("dart.function=true;dart.name={name}")
            };
            symbols.push(symbol(
                &input,
                &language,
                name,
                NodeKind::Function,
                line_number,
                metadata,
            ));
        }
        if trimmed.contains("routes:") || trimmed.contains("GoRoute(") {
            if let Some(route) =
                literal_after(trimmed, "path:").or_else(|| literal_after(trimmed, "name:"))
            {
                symbols.push(symbol(
                    &input,
                    &language,
                    format!("Flutter route {route}"),
                    NodeKind::Route,
                    line_number,
                    format!(
                        "route.framework=flutter;route.kind=ClientRoute;route.method=GET;route.path={};route.file={};route.source=FlutterRouteLiteral;route.line_start={line_number};route.line_end={line_number};route.confidence=7500",
                        normalize_route_path("", &route),
                        normalized_file(&input)
                    ),
                ));
            }
        }
        if trimmed.contains(".get(") || trimmed.contains(".post(") {
            if let Some(url) = literal_in(trimmed) {
                symbols.push(symbol(
                    &input,
                    &language,
                    format!("Dart HTTP {url}"),
                    NodeKind::Route,
                    line_number,
                    format!(
                        "route.framework=dart;route.kind=HttpClientCall;route.method=GET;route.path={};route.file={};route.source=DartHttpLiteral;route.line_start={line_number};route.line_end={line_number};route.confidence=7000",
                        url,
                        normalized_file(&input)
                    ),
                ));
            }
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

fn function_name(line: &str) -> Option<String> {
    if !line.contains('(') || line.starts_with("if ") || line.starts_with("for ") {
        return None;
    }
    let before = line.split_once('(')?.0.trim();
    before
        .rsplit(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .find(|value| !value.is_empty())
        .map(str::to_string)
}
