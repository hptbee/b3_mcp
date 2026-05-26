use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input, "html")];
    if let Some(title) = title(&input.source) {
        symbols.push(symbol(
            &input,
            "html",
            title.clone(),
            NodeKind::ConfigKey,
            1,
            format!("html.title={title};html.file={}", normalized_file(&input)),
        ));
    }
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        for attr in ["id", "class", "data-testid", "data-controller"] {
            for value in attr_values(line, attr) {
                symbols.push(symbol(
                    &input,
                    "html",
                    format!("{attr}={value}"),
                    NodeKind::ConfigKey,
                    line_number,
                    format!(
                        "html.attribute={attr};html.value={value};html.file={}",
                        normalized_file(&input)
                    ),
                ));
            }
        }
        for attr in ["src", "href", "action"] {
            for value in attr_values(line, attr) {
                let kind = if attr == "action" || (attr == "href" && value.starts_with('/')) {
                    NodeKind::Route
                } else {
                    NodeKind::Package
                };
                symbols.push(symbol(
                    &input,
                    "html",
                    value.clone(),
                    kind,
                    line_number,
                    if kind == NodeKind::Route {
                        format!(
                            "route.framework=html;route.kind=ClientRoute;route.method={};route.path={};route.file={};route.source=HtmlReference;route.line_start={line_number};route.line_end={line_number};route.confidence=7000",
                            form_method(line).unwrap_or_else(|| "GET".to_string()),
                            normalize_route_path("", &value),
                            normalized_file(&input)
                        )
                    } else {
                        format!("html.reference=true;html.attribute={attr};html.path={value};html.file={}", normalized_file(&input))
                    },
                ));
            }
        }
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("html".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

fn title(source: &str) -> Option<String> {
    let start = source.find("<title>")? + "<title>".len();
    let rest = &source[start..];
    let end = rest.find("</title>")?;
    Some(rest[..end].trim().to_string())
}

fn attr_values(line: &str, attr: &str) -> Vec<String> {
    let mut values = Vec::new();
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        let mut rest = line;
        while let Some(index) = rest.find(&needle) {
            rest = &rest[index + needle.len()..];
            let Some(end) = rest.find(quote) else {
                break;
            };
            values.extend(rest[..end].split_whitespace().map(str::to_string));
            rest = &rest[end + 1..];
        }
    }
    values
}

fn form_method(line: &str) -> Option<String> {
    attr_values(line, "method")
        .first()
        .map(|value| value.to_ascii_uppercase())
}
