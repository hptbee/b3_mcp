use super::*;

mod ksqldb;
mod sql;

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
