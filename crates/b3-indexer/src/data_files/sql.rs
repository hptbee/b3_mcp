use super::*;

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    let mut symbols = vec![module_symbol(&input)];
    let migration = input
        .path
        .to_string_lossy()
        .to_ascii_lowercase()
        .contains("migration");
    for (index, statement) in input.source.split(';').enumerate() {
        let line = line_for_statement(&input.source, statement).unwrap_or(index + 1);
        let upper = statement.to_ascii_uppercase();
        for (marker, kind) in [
            ("CREATE TABLE", "Table"),
            ("CREATE VIEW", "View"),
            ("CREATE PROCEDURE", "Procedure"),
            ("CREATE FUNCTION", "Function"),
        ] {
            if let Some(name) = word_after(&upper, statement, marker) {
                symbols.push(sql_symbol(
                    &input,
                    &name,
                    line,
                    format!(
                        "data_access.technology=sql;data_access.kind={kind};data_access.entity={name};data_access.operation=create;data_access.file={};data_access.source=SqlDefinition;data_access.line_start={line};data_access.line_end={line};data_access.confidence={};sql.migration={migration}",
                        normalized_file(&input),
                        if migration { 8500 } else { 8000 }
                    ),
                ));
            }
        }
        for (marker, operation) in [
            ("INSERT INTO", "insert"),
            ("UPDATE", "update"),
            ("DELETE FROM", "delete"),
            ("FROM", "select"),
            ("JOIN", "join"),
        ] {
            for name in words_after_all(&upper, statement, marker) {
                symbols.push(sql_symbol(
                    &input,
                    &name,
                    line,
                    format!(
                        "data_access.technology=sql;data_access.kind=TableReference;data_access.entity={name};data_access.operation={operation};data_access.file={};data_access.source=SqlTableReference;data_access.line_start={line};data_access.line_end={line};data_access.confidence=7000;sql.migration={migration}",
                        normalized_file(&input)
                    ),
                ));
            }
        }
    }
    Ok(ParsedFile {
        file_id: input.file_id,
        language: Some("sql".to_string()),
        symbols,
        relationships: Vec::new(),
    })
}

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:sql-module", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("sql")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line: input.source.lines().count().max(1),
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: Some("sql.file=true".to_string()),
    }
}

fn sql_symbol(input: &ParseInput, name: &str, line: usize, metadata: String) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:sql:{name}:{line}", input.file_id.as_str()),
        )),
        file_id: input.file_id.clone(),
        name: name.to_string(),
        kind: NodeKind::Endpoint,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: line,
        start_column: 0,
        end_line: line,
        end_column: 0,
        visibility: Some(metadata),
    }
}

fn word_after(upper: &str, original: &str, marker: &str) -> Option<String> {
    let index = upper.find(marker)? + marker.len();
    original[index..]
        .trim()
        .trim_start_matches("IF NOT EXISTS")
        .trim()
        .split(|ch: char| !(ch == '_' || ch == '.' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_matches('"').trim_matches('`').to_string())
}

fn words_after_all(upper: &str, original: &str, marker: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut offset = 0usize;
    while let Some(index) = upper[offset..].find(marker) {
        let start = offset + index + marker.len();
        if let Some(name) = word_after(
            &upper[start - marker.len()..],
            &original[start - marker.len()..],
            marker,
        ) {
            names.push(name);
        }
        offset = start;
    }
    names.sort();
    names.dedup();
    names
}

fn line_for_statement(source: &str, statement: &str) -> Option<usize> {
    let offset = source.find(statement.trim())?;
    Some(source[..offset].lines().count().max(1))
}
