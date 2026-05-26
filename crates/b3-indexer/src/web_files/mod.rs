use super::*;

mod css_scss;
mod html;
pub(crate) mod threejs_webgl;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    match language_from_path(&input.path).as_deref() {
        Some("html") => html::parse(input),
        Some("css" | "scss") => css_scss::parse(input),
        _ => NoopTreeSitterParser.parse(input),
    }
}

pub(crate) fn module_symbol(input: &ParseInput, language: &str) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}-web-file:{}",
                input.file_id.as_str(),
                input.path.display()
            ),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("web-file")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line: input.source.lines().count().max(1),
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some(format!("{language}.file=true;web_asset.file=true")),
    }
}

pub(crate) fn symbol(
    input: &ParseInput,
    language: &str,
    name: impl Into<String>,
    kind: NodeKind,
    line: usize,
    metadata: String,
) -> ExtractedSymbol {
    let name = name.into();
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:{language}:web:{name}:{line}", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name,
        kind,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: line,
        end_column: 0,
        visibility: Some(metadata),
    }
}

pub(crate) fn literal_after(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let rest = line.get(start..)?.trim_start();
    let quote = rest.chars().next().filter(|ch| *ch == '"' || *ch == '\'')?;
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(crate) fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn normalize_route_path(base: &str, path: &str) -> String {
    let clean_base = base.trim_matches('/');
    let clean_path = path.trim_matches('/');
    match (clean_base.is_empty(), clean_path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{clean_path}"),
        (false, true) => format!("/{clean_base}"),
        (false, false) => format!("/{clean_base}/{clean_path}"),
    }
}
