use super::*;

mod c_cpp;
mod dart;
mod objective_c;
mod swift;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    match language_from_path(&input.path).as_deref() {
        Some(
            "c" | "c_header" | "cpp" | "cpp_header" | "cmake" | "makefile" | "compile_commands",
        ) => c_cpp::parse(input),
        Some("swift" | "swift_project") => swift::parse(input),
        Some("objective_c" | "objective_cpp") => objective_c::parse(input),
        Some("dart" | "dart_project") => dart::parse(input),
        _ => NoopTreeSitterParser.parse(input),
    }
}

pub(crate) fn module_symbol(input: &ParseInput, language: &str) -> ExtractedSymbol {
    let end_line = input.source.lines().count().max(1);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}-module:{}",
                input.file_id.as_str(),
                input.path.display()
            ),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("source")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line,
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some(format!("{language}.file=true")),
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
    let (start_byte, end_byte, end_line, end_column) = line_span(input, line);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:{language}:{}:{}:{line}",
                input.file_id.as_str(),
                node_kind_name(kind),
                name
            ),
        )),
        file_id: input.file_id.clone(),
        name,
        kind,
        start_byte,
        end_byte,
        start_line: line,
        start_column: 0,
        end_line,
        end_column,
        visibility: Some(metadata),
    }
}

pub(crate) fn package_symbol(
    input: &ParseInput,
    language: &str,
    name: impl Into<String>,
    line: usize,
    metadata: String,
) -> ExtractedSymbol {
    symbol(input, language, name, NodeKind::Package, line, metadata)
}

pub(crate) fn relationships(symbols: &[ExtractedSymbol]) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    relationships
}

pub(crate) fn identifier_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let start = line.find(marker)? + marker.len();
    line.get(start..)?
        .trim_start()
        .trim_start_matches(|ch: char| ch == '(' || ch == '+' || ch == '-')
        .split(|ch: char| !(ch == '_' || ch == ':' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|value| !value.is_empty())
}

pub(crate) fn literal_after(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    literal_in(line.get(start..)?)
}

pub(crate) fn literal_in(value: &str) -> Option<String> {
    let start = value.find(['"', '\''])?;
    let quote = value.as_bytes().get(start).copied()? as char;
    let rest = &value[start + 1..];
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

pub(crate) fn line_span(input: &ParseInput, line: usize) -> (usize, usize, usize, usize) {
    let mut offset = 0usize;
    for (index, text) in input.source.lines().enumerate() {
        let current = index + 1;
        let next = offset + text.len() + 1;
        if current == line {
            return (offset, next.min(input.source.len()), line, text.len());
        }
        offset = next;
    }
    (0, input.source.len(), line.max(1), 0)
}
