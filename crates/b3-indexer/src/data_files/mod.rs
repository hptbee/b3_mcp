use super::*;

mod ksqldb;
mod sql;

pub(crate) fn line_for_statement(source: &str, statement: &str) -> Option<usize> {
    let offset = source.find(statement.trim())?;
    Some(source[..offset].lines().count().max(1))
}

pub(crate) fn strip_sql_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("--") {
                ""
            } else if let Some(index) = line.find("--") {
                &line[..index]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn parse(input: ParseInput) -> ContractResult<ParsedFile> {
    if is_ksqldb_file(&input.path, &input.source) {
        ksqldb::parse(input)
    } else if language_from_path(&input.path).as_deref() == Some("sql") {
        sql::parse(input)
    } else {
        NoopTreeSitterParser.parse(input)
    }
}

pub(crate) fn is_ksqldb_file(path: &Path, source: &str) -> bool {
    ksqldb::is_ksqldb_file(path, source)
}

pub(crate) fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}
