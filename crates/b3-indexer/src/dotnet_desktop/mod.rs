//! Basic static .NET Desktop / WPF intelligence.

mod mvvm;
mod wpf;
mod xaml;

use std::path::Path;

use b3_core::{ContractResult, NodeKind, SymbolId};

use crate::{ExtractedRelationship, ExtractedSymbol, ParseInput, ParsedFile};

pub use wpf::detect_wpf_project_technologies;
pub(crate) use wpf::is_dotnet_desktop_file;
#[cfg(test)]
pub(crate) use wpf::wpf_metadata_value;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let path = input.path.clone();
    let symbols = if is_xaml_file(&path) {
        xaml::extract_xaml_symbols(&input)
    } else if is_csproj_file(&path) {
        wpf::extract_project_symbols(&input)
    } else if is_csharp_file(&path) {
        mvvm::extract_csharp_symbols(&input)
    } else {
        Vec::new()
    };

    Ok(ParsedFile {
        file_id: input.file_id,
        language: language_for_path(&path),
        symbols,
        relationships: Vec::<ExtractedRelationship>::new(),
    })
}

pub(crate) fn is_xaml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xaml"))
}

pub(crate) fn is_csproj_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csproj"))
}

pub(crate) fn is_csharp_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("cs"))
}

fn language_for_path(path: &Path) -> Option<String> {
    if is_xaml_file(path) {
        Some("xaml".to_string())
    } else if is_csproj_file(path) {
        Some("csproj".to_string())
    } else if is_csharp_file(path) {
        Some("csharp".to_string())
    } else {
        None
    }
}

fn symbol(
    input: &ParseInput,
    name: impl Into<String>,
    kind: NodeKind,
    source: &str,
    line_start: usize,
    line_end: usize,
    metadata: String,
) -> ExtractedSymbol {
    let name = name.into();
    let start_byte = line_start.saturating_sub(1);
    ExtractedSymbol {
        id: SymbolId::new(crate::stable_id(
            "symbol",
            &format!(
                "{}:{name}:{source}:{line_start}:{line_end}",
                input.file_id.as_str()
            ),
        )),
        file_id: input.file_id.clone(),
        name,
        kind,
        start_byte,
        end_byte: start_byte,
        start_line: line_start.max(1),
        start_column: 0,
        end_line: line_end.max(line_start).max(1),
        end_column: 0,
        visibility: Some(metadata),
    }
}

fn metadata(fields: &[(&str, String)]) -> String {
    fields
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("wpf.{key}={}", sanitize_metadata_value(value)))
        .collect::<Vec<_>>()
        .join(";")
}

fn sanitize_metadata_value(value: &str) -> String {
    value
        .replace(';', ",")
        .replace('\n', " ")
        .replace('\r', " ")
}

fn file_path(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}

fn line_of(source: &str, needle: &str) -> usize {
    source
        .find(needle)
        .map(|index| {
            source[..index]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1
        })
        .unwrap_or(1)
}

fn local_type_name(value: &str) -> String {
    value
        .rsplit(['.', ':'])
        .next()
        .unwrap_or(value)
        .trim()
        .trim_end_matches("View")
        .to_string()
}
