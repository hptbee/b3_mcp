use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let language = language_from_path(&input.path).unwrap_or_else(|| "swift".to_string());
    let mut symbols = vec![module_symbol(&input, &language)];
    if language == "swift_project" {
        symbols.push(symbol(
            &input,
            &language,
            "Swift Package",
            NodeKind::Package,
            1,
            "swift.project=true;swift.support=Basic".to_string(),
        ));
    }
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("import ") {
            if let Some(name) = trimmed
                .strip_prefix("import ")
                .and_then(|v| v.split_whitespace().next())
            {
                symbols.push(package_symbol(
                    &input,
                    &language,
                    name,
                    line_number,
                    format!("swift.import=true;swift.import_path={name}"),
                ));
            }
        }
        for (prefix, kind, key) in [
            ("class ", NodeKind::Class, "class"),
            ("struct ", NodeKind::Struct, "struct"),
            ("enum ", NodeKind::Enum, "enum"),
            ("protocol ", NodeKind::Interface, "protocol"),
            ("extension ", NodeKind::Class, "extension"),
        ] {
            if let Some(name) = identifier_after(trimmed, prefix) {
                let mut metadata = format!("swift.{key}=true;swift.name={name}");
                if trimmed.contains(": View")
                    || input
                        .source
                        .lines()
                        .skip(index)
                        .take(8)
                        .any(|l| l.contains("var body: some View"))
                {
                    metadata.push_str(";swift.swiftui_view=true");
                }
                if trimmed.contains("@main")
                    || input
                        .source
                        .lines()
                        .take(index + 1)
                        .any(|l| l.trim() == "@main")
                {
                    metadata.push_str(";swift.app_entry=true");
                }
                symbols.push(symbol(&input, &language, name, kind, line_number, metadata));
            }
        }
        if let Some(name) = identifier_after(trimmed, "func ") {
            symbols.push(symbol(
                &input,
                &language,
                name,
                NodeKind::Function,
                line_number,
                format!("swift.function=true;swift.name={name}"),
            ));
        }
        if trimmed.contains("URLSession") {
            if let Some(url) = literal_after(trimmed, "URL(string:") {
                symbols.push(symbol(
                    &input,
                    &language,
                    format!("URLSession {url}"),
                    NodeKind::Route,
                    line_number,
                    format!(
                        "route.framework=swift;route.kind=HttpClientCall;route.method=GET;route.path={};route.file={};route.source=SwiftURLSession;route.line_start={line_number};route.line_end={line_number};route.confidence=7000",
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
