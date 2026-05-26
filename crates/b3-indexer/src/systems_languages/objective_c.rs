use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let language = language_from_path(&input.path).unwrap_or_else(|| "objective_c".to_string());
    let mut symbols = vec![module_symbol(&input, &language)];
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("#import") || trimmed.starts_with("#include") {
            if let Some(import) = literal_in(trimmed).or_else(|| angle_literal(trimmed)) {
                symbols.push(package_symbol(
                    &input,
                    &language,
                    import.clone(),
                    line_number,
                    format!("objective_c.import=true;objective_c.import_path={import}"),
                ));
            }
        }
        for (prefix, kind, key) in [
            ("@interface ", NodeKind::Class, "interface"),
            ("@implementation ", NodeKind::Class, "implementation"),
            ("@protocol ", NodeKind::Interface, "protocol"),
        ] {
            if let Some(name) = identifier_after(trimmed, prefix) {
                let mut metadata = format!("objective_c.{key}=true;objective_c.name={name}");
                if trimmed.contains("UIViewController") {
                    metadata.push_str(";objective_c.uiviewcontroller=true");
                }
                symbols.push(symbol(&input, &language, name, kind, line_number, metadata));
            }
        }
        if (trimmed.starts_with("- (") || trimmed.starts_with("+ (")) && trimmed.contains(')') {
            if let Some(name) = method_name(trimmed) {
                symbols.push(symbol(
                    &input,
                    &language,
                    name.clone(),
                    NodeKind::Method,
                    line_number,
                    format!("objective_c.method=true;objective_c.name={name}"),
                ));
            }
        }
        if trimmed.starts_with("@property") {
            if let Some(name) = trimmed
                .trim_end_matches(';')
                .rsplit(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
                .find(|value| !value.is_empty())
            {
                symbols.push(symbol(
                    &input,
                    &language,
                    name,
                    NodeKind::Variable,
                    line_number,
                    format!("objective_c.property=true;objective_c.name={name}"),
                ));
            }
        }
        if trimmed.contains("NSURLSession") {
            if let Some(url) = literal_after(trimmed, "URLWithString:") {
                symbols.push(symbol(
                    &input,
                    &language,
                    format!("NSURLSession {url}"),
                    NodeKind::Route,
                    line_number,
                    format!(
                        "route.framework=objective_c;route.kind=HttpClientCall;route.method=GET;route.path={};route.file={};route.source=ObjCNSURLSession;route.line_start={line_number};route.line_end={line_number};route.confidence=7000",
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

fn angle_literal(line: &str) -> Option<String> {
    let start = line.find('<')?;
    let rest = &line[start + 1..];
    let end = rest.find('>')?;
    Some(rest[..end].to_string())
}

fn method_name(line: &str) -> Option<String> {
    let after_return = line.split_once(')')?.1.trim();
    after_return
        .split_whitespace()
        .next()
        .map(|value| value.trim_end_matches(':').to_string())
        .filter(|value| !value.is_empty())
}
