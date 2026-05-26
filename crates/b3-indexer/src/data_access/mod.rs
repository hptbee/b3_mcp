use super::*;

const READ_METHODS: &[&str] = &[
    "Where",
    "First",
    "FirstAsync",
    "FirstOrDefault",
    "FirstOrDefaultAsync",
    "Single",
    "SingleAsync",
    "SingleOrDefault",
    "SingleOrDefaultAsync",
    "ToList",
    "ToListAsync",
    "Find",
    "FindAsync",
    "Any",
    "AnyAsync",
    "Count",
    "CountAsync",
];

pub fn detect_csproj_data_access_technologies(
    source: &str,
) -> ContractResult<Vec<DetectedTechnology>> {
    let lower = source.to_ascii_lowercase();
    let mut detected = Vec::new();
    if lower.contains("microsoft.entityframeworkcore") {
        detected.push(data_access_technology(
            "ef_core",
            "Entity Framework Core",
            "csproj",
        ));
    }
    if lower.contains("dapper") {
        detected.push(data_access_technology("dapper", "Dapper", "csproj"));
    }
    Ok(detected)
}

pub fn detect_package_json_data_access_technologies(
    source: &str,
) -> ContractResult<Vec<DetectedTechnology>> {
    let value = serde_json::from_str::<serde_json::Value>(source)
        .map_err(|error| ContractError::new(format!("invalid package.json: {error}")))?;
    let mut detected = Vec::new();
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        let Some(object) = value.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for package_name in object.keys() {
            let technology = match package_name.as_str() {
                "@prisma/client" | "prisma" => Some(("prisma", "Prisma")),
                "typeorm" => Some(("typeorm", "TypeORM")),
                "sequelize" | "sequelize-typescript" => Some(("sequelize", "Sequelize")),
                "mysql2" | "pg" | "mssql" => Some(("raw_sql", "SQL Driver")),
                _ => None,
            };
            if let Some((id, name)) = technology {
                if !detected
                    .iter()
                    .any(|existing: &DetectedTechnology| existing.id == id)
                {
                    detected.push(data_access_technology(
                        id,
                        name,
                        &format!("package.json:{section}:{package_name}"),
                    ));
                }
            }
        }
    }
    Ok(detected)
}

pub(crate) fn collect_csharp_data_access(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    let lines: Vec<&str> = input.source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "using Microsoft.EntityFrameworkCore;" {
            output.push(data_access_symbol(
                input,
                "EF Core using",
                line_number,
                DataAccessMetadata {
                    technology: "ef_core".to_string(),
                    kind: "Client".to_string(),
                    file_path: normalized_file(input),
                    symbol_id: containing_symbol_id(symbols, line_number),
                    class_name: None,
                    method_name: None,
                    entity_name: None,
                    context_name: None,
                    repository_name: None,
                    operation: None,
                    query_text: None,
                    line_start: line_number,
                    line_end: line_number,
                    confidence: 8_000,
                    source_kind: "EfCoreUsing".to_string(),
                },
            ));
        }
        if trimmed == "using Dapper;" {
            output.push(data_access_symbol(
                input,
                "Dapper using",
                line_number,
                DataAccessMetadata {
                    technology: "dapper".to_string(),
                    kind: "Client".to_string(),
                    file_path: normalized_file(input),
                    symbol_id: containing_symbol_id(symbols, line_number),
                    class_name: None,
                    method_name: None,
                    entity_name: None,
                    context_name: None,
                    repository_name: None,
                    operation: None,
                    query_text: None,
                    line_start: line_number,
                    line_end: line_number,
                    confidence: 8_000,
                    source_kind: "DapperUsing".to_string(),
                },
            ));
        }
        if let Some((class_name, context_name)) = ef_core_db_context(trimmed) {
            output.push(data_access_symbol(
                input,
                &format!("EF Core DbContext {context_name}"),
                line_number,
                DataAccessMetadata {
                    technology: "ef_core".to_string(),
                    kind: "DbContext".to_string(),
                    file_path: normalized_file(input),
                    symbol_id: containing_symbol_id(symbols, line_number),
                    class_name: Some(class_name),
                    method_name: None,
                    entity_name: None,
                    context_name: Some(context_name),
                    repository_name: None,
                    operation: None,
                    query_text: None,
                    line_start: line_number,
                    line_end: line_number,
                    confidence: 9_500,
                    source_kind: "EfCoreDbContext".to_string(),
                },
            ));
        }
        if let Some((entity, dbset)) = ef_core_db_set(trimmed) {
            output.push(data_access_symbol(
                input,
                &format!("EF Core DbSet {dbset}"),
                line_number,
                DataAccessMetadata {
                    technology: "ef_core".to_string(),
                    kind: "DbSet".to_string(),
                    file_path: normalized_file(input),
                    symbol_id: containing_symbol_id(symbols, line_number),
                    class_name: containing_class_name(symbols, line_number),
                    method_name: None,
                    entity_name: Some(entity),
                    context_name: containing_class_name(symbols, line_number),
                    repository_name: None,
                    operation: None,
                    query_text: None,
                    line_start: line_number,
                    line_end: line_number,
                    confidence: 9_500,
                    source_kind: "EfCoreDbSet".to_string(),
                },
            ));
        }
        if let Some((operation, entity, source_kind)) = ef_core_callsite(trimmed) {
            output.push(data_access_symbol(
                input,
                &format!(
                    "EF Core {operation} {}",
                    entity.as_deref().unwrap_or("context")
                ),
                line_number,
                DataAccessMetadata {
                    technology: "ef_core".to_string(),
                    kind: "QueryCall".to_string(),
                    file_path: normalized_file(input),
                    symbol_id: containing_symbol_id(symbols, line_number),
                    class_name: containing_class_name(symbols, line_number),
                    method_name: containing_method_name(symbols, line_number),
                    entity_name: entity,
                    context_name: None,
                    repository_name: None,
                    operation: Some(operation),
                    query_text: None,
                    line_start: line_number,
                    line_end: line_number,
                    confidence: 8_500,
                    source_kind,
                },
            ));
        }
        if let Some((operation, entity, sql, source_kind)) = dapper_callsite(trimmed) {
            output.push(data_access_symbol(
                input,
                &format!(
                    "Dapper {operation} {}",
                    entity.unwrap_or_else(|| "SQL".to_string())
                ),
                line_number,
                DataAccessMetadata {
                    technology: "dapper".to_string(),
                    kind: if operation == "execute" {
                        "ExecuteCall".to_string()
                    } else {
                        "QueryCall".to_string()
                    },
                    file_path: normalized_file(input),
                    symbol_id: containing_symbol_id(symbols, line_number),
                    class_name: containing_class_name(symbols, line_number),
                    method_name: containing_method_name(symbols, line_number),
                    entity_name: None,
                    context_name: None,
                    repository_name: None,
                    operation: Some(operation),
                    query_text: sql,
                    line_start: line_number,
                    line_end: line_number,
                    confidence: 9_000,
                    source_kind,
                },
            ));
        }
    }
    output
}

pub(crate) fn collect_web_data_access(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut output = Vec::new();
    let lines: Vec<&str> = input.source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.contains("@prisma/client") {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                "Prisma import",
                "prisma",
                "Client",
                None,
                None,
                None,
                8_000,
                "PrismaImport",
            ));
        }
        if trimmed.contains("\"typeorm\"") || trimmed.contains("'typeorm'") {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                "TypeORM import",
                "typeorm",
                "Repository",
                None,
                None,
                None,
                8_000,
                "TypeOrmImport",
            ));
        }
        if trimmed.contains("\"sequelize\"") || trimmed.contains("'sequelize'") {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                "Sequelize import",
                "sequelize",
                "Client",
                None,
                None,
                None,
                8_000,
                "SequelizeImport",
            ));
        }
        if trimmed.contains("new PrismaClient(") {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                "Prisma Client",
                "prisma",
                "Client",
                None,
                None,
                None,
                9_500,
                "PrismaClientConstruction",
            ));
        }
        if let Some((model, operation)) = prisma_callsite(trimmed) {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                &format!("Prisma {operation} {model}"),
                "prisma",
                if operation == "raw_sql" {
                    "ExecuteCall"
                } else {
                    "QueryCall"
                },
                Some(operation),
                Some(model),
                None,
                8_500,
                "PrismaClientCall",
            ));
        }
        if trimmed.contains("@Entity") {
            if let Some(class_name) = next_class_name(&lines, index) {
                output.push(web_metadata_symbol(
                    input,
                    symbols,
                    line_number,
                    &format!("TypeORM Entity {class_name}"),
                    "typeorm",
                    "Entity",
                    None,
                    Some(class_name),
                    None,
                    9_000,
                    "TypeOrmEntityDecorator",
                ));
            }
        }
        if let Some((entity, operation)) = typeorm_callsite(trimmed) {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                &format!(
                    "TypeORM {operation} {}",
                    entity.as_deref().unwrap_or("repository")
                ),
                "typeorm",
                "Repository",
                Some(operation),
                entity,
                None,
                8_000,
                "TypeOrmRepositoryCall",
            ));
        }
        if let Some(model) = sequelize_model(trimmed) {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                &format!("Sequelize Model {model}"),
                "sequelize",
                "Model",
                None,
                Some(model),
                None,
                9_000,
                "SequelizeModel",
            ));
        }
        if let Some((model, operation)) = sequelize_callsite(trimmed) {
            output.push(web_metadata_symbol(
                input,
                symbols,
                line_number,
                &format!("Sequelize {operation} {model}"),
                "sequelize",
                "QueryCall",
                Some(operation),
                Some(model),
                None,
                8_500,
                "SequelizeModelCall",
            ));
        }
    }
    output
}

fn data_access_technology(id: &str, name: &str, source: &str) -> DetectedTechnology {
    DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind: TechnologyKind::Orm,
        support_level: TechnologySupportLevel::Basic,
        capabilities: vec![
            TechnologyCapability::DetectPackage,
            TechnologyCapability::DetectImport,
        ],
        source: source.to_string(),
    }
}

fn ef_core_db_context(line: &str) -> Option<(String, String)> {
    if !(line.contains(" class ") && line.contains(":") && line.contains("DbContext")) {
        return None;
    }
    let class_name = line
        .split(" class ")
        .nth(1)?
        .split([' ', ':', '{'])
        .next()?;
    Some((class_name.to_string(), class_name.to_string()))
}

fn ef_core_db_set(line: &str) -> Option<(String, String)> {
    let start = line.find("DbSet<")? + "DbSet<".len();
    let entity = line[start..].split('>').next()?.trim();
    let after = line.split('>').nth(1)?.trim();
    let dbset = after
        .split_whitespace()
        .find(|part| is_identifier(part.trim_matches(['{', ';'])))?
        .trim_matches(['{', ';'])
        .to_string();
    Some((entity.to_string(), dbset))
}

fn ef_core_callsite(line: &str) -> Option<(String, Option<String>, String)> {
    if line.contains("SaveChangesAsync") || line.contains("SaveChanges(") {
        return Some(("execute".to_string(), None, "EfCoreSaveChanges".to_string()));
    }
    for method in READ_METHODS {
        let needle = format!(".{method}(");
        if line.contains(&needle) {
            return Some((
                "read".to_string(),
                receiver_before_method(line, method),
                "EfCoreLinqQuery".to_string(),
            ));
        }
    }
    for (method, operation) in [
        ("AddAsync", "insert"),
        ("Add", "insert"),
        ("Update", "update"),
        ("Remove", "delete"),
    ] {
        if line.contains(&format!(".{method}(")) {
            return Some((
                operation.to_string(),
                receiver_before_method(line, method),
                "EfCoreChangeTrackingCall".to_string(),
            ));
        }
    }
    None
}

fn dapper_callsite(line: &str) -> Option<(String, Option<String>, Option<String>, String)> {
    let method = [
        "QueryFirstOrDefaultAsync",
        "QueryFirstOrDefault",
        "QuerySingleAsync",
        "QuerySingle",
        "QueryAsync",
        "Query",
        "ExecuteAsync",
        "Execute",
    ]
    .into_iter()
    .find(|method| line.contains(&format!(".{method}")))?;
    let operation = if method.starts_with("Query") {
        "read"
    } else {
        "execute"
    };
    let entity = generic_type_after(line, method);
    let sql = literal_string_argument(line);
    Some((
        operation.to_string(),
        entity,
        sql,
        if method.starts_with("Query") {
            "DapperQuery".to_string()
        } else {
            "DapperExecute".to_string()
        },
    ))
}

fn prisma_callsite(line: &str) -> Option<(String, String)> {
    let prisma_pos = line.find("prisma.")?;
    let after = &line[prisma_pos + "prisma.".len()..];
    if after.starts_with("$queryRaw") || after.starts_with("$executeRaw") {
        return Some(("raw_sql".to_string(), "raw_sql".to_string()));
    }
    let (model, rest) = after.split_once('.')?;
    let operation_name = rest.split('(').next()?.trim();
    let operation = match operation_name {
        "findMany" | "findUnique" | "findFirst" | "count" | "aggregate" => "read",
        "create" | "createMany" => "insert",
        "update" | "updateMany" | "upsert" => "update",
        "delete" | "deleteMany" => "delete",
        _ => return None,
    };
    Some((model.to_string(), operation.to_string()))
}

fn typeorm_callsite(line: &str) -> Option<(Option<String>, String)> {
    if let Some(entity) = argument_of(line, "getRepository") {
        return Some((Some(entity), "read".to_string()));
    }
    if let Some(entity) = argument_of(line, "manager.find") {
        return Some((Some(entity), "read".to_string()));
    }
    for (method, operation) in [
        ("findOne", "read"),
        ("find", "read"),
        ("save", "insert"),
        ("update", "update"),
        ("delete", "delete"),
        ("remove", "delete"),
    ] {
        if line.contains(&format!(".{method}(")) {
            return Some((None, operation.to_string()));
        }
    }
    None
}

fn sequelize_model(line: &str) -> Option<String> {
    if line.contains(" extends Model") {
        return line
            .split("class ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .map(str::to_string);
    }
    if line.contains(".define(") || line.contains("sequelize.define(") {
        return literal_string_argument(line);
    }
    None
}

fn sequelize_callsite(line: &str) -> Option<(String, String)> {
    for (method, operation) in [
        ("findAll", "read"),
        ("findOne", "read"),
        ("create", "insert"),
        ("update", "update"),
        ("destroy", "delete"),
    ] {
        let needle = format!(".{method}(");
        if let Some(pos) = line.find(&needle) {
            let model = line[..pos]
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .filter(|part| !part.is_empty())
                .next_back()?;
            return Some((model.to_string(), operation.to_string()));
        }
    }
    None
}

fn web_metadata_symbol(
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    line: usize,
    name: &str,
    technology: &str,
    kind: &str,
    operation: Option<String>,
    entity_name: Option<String>,
    query_text: Option<String>,
    confidence: u16,
    source_kind: &str,
) -> ExtractedSymbol {
    data_access_symbol(
        input,
        name,
        line,
        DataAccessMetadata {
            technology: technology.to_string(),
            kind: kind.to_string(),
            file_path: normalized_file(input),
            symbol_id: containing_symbol_id(symbols, line),
            class_name: containing_class_name(symbols, line),
            method_name: containing_method_name(symbols, line),
            entity_name,
            context_name: None,
            repository_name: None,
            operation,
            query_text,
            line_start: line,
            line_end: line,
            confidence,
            source_kind: source_kind.to_string(),
        },
    )
}

fn data_access_symbol(
    input: &ParseInput,
    name: &str,
    line: usize,
    metadata: DataAccessMetadata,
) -> ExtractedSymbol {
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!(
                "{}:data-access:{}:{}:{}",
                input.file_id.as_str(),
                metadata.technology,
                name,
                line
            ),
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
        visibility: Some(encode_data_access_metadata(&metadata)),
    }
}

pub(crate) fn encode_data_access_metadata(metadata: &DataAccessMetadata) -> String {
    [
        ("data_access.technology", Some(metadata.technology.as_str())),
        ("data_access.kind", Some(metadata.kind.as_str())),
        ("data_access.file", Some(metadata.file_path.as_str())),
        ("data_access.class", metadata.class_name.as_deref()),
        ("data_access.method", metadata.method_name.as_deref()),
        ("data_access.entity", metadata.entity_name.as_deref()),
        ("data_access.context", metadata.context_name.as_deref()),
        (
            "data_access.repository",
            metadata.repository_name.as_deref(),
        ),
        ("data_access.operation", metadata.operation.as_deref()),
        ("data_access.query", metadata.query_text.as_deref()),
        ("data_access.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", escape_metadata(value))))
    .chain([
        format!("data_access.line_start={}", metadata.line_start),
        format!("data_access.line_end={}", metadata.line_end),
        format!("data_access.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

#[cfg(test)]
pub(crate) fn data_access_metadata_value(metadata: &str, key: &str) -> Option<String> {
    prefixed_metadata_value(metadata, "data_access", key)
}

fn containing_symbol_id(symbols: &[ExtractedSymbol], line: usize) -> Option<SymbolId> {
    symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                NodeKind::Method | NodeKind::Function | NodeKind::Class
            ) && symbol.start_line <= line
                && symbol.end_line >= line
        })
        .min_by_key(|symbol| symbol.end_line.saturating_sub(symbol.start_line))
        .map(|symbol| symbol.id.clone())
}

fn containing_method_name(symbols: &[ExtractedSymbol], line: usize) -> Option<String> {
    containing_symbol(symbols, line, &[NodeKind::Method, NodeKind::Function])
        .map(|symbol| symbol.name.clone())
}

fn containing_class_name(symbols: &[ExtractedSymbol], line: usize) -> Option<String> {
    containing_symbol(symbols, line, &[NodeKind::Class]).map(|symbol| symbol.name.clone())
}

fn containing_symbol<'a>(
    symbols: &'a [ExtractedSymbol],
    line: usize,
    kinds: &[NodeKind],
) -> Option<&'a ExtractedSymbol> {
    symbols
        .iter()
        .filter(|symbol| kinds.contains(&symbol.kind))
        .filter(|symbol| symbol.start_line <= line && symbol.end_line >= line)
        .min_by_key(|symbol| symbol.end_line.saturating_sub(symbol.start_line))
}

fn receiver_before_method(line: &str, method: &str) -> Option<String> {
    let pos = line.find(&format!(".{method}"))?;
    line[..pos]
        .split('.')
        .next_back()
        .map(|value| value.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_'))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn generic_type_after(line: &str, method: &str) -> Option<String> {
    let start = line.find(&format!(".{method}<"))? + method.len() + 2;
    let rest = &line[start..];
    Some(rest.split('>').next()?.trim().to_string()).filter(|value| !value.is_empty())
}

fn literal_string_argument(line: &str) -> Option<String> {
    let quote = line.find(['"', '\''])?;
    let quote_char = line.as_bytes().get(quote).copied()? as char;
    let after = &line[quote + 1..];
    let end = after.find(quote_char)?;
    Some(after[..end].to_string())
}

fn argument_of(line: &str, function_name: &str) -> Option<String> {
    let start = line.find(&format!("{function_name}("))? + function_name.len() + 1;
    Some(
        line[start..]
            .split([')', ',', ' '])
            .next()
            .unwrap_or_default()
            .trim()
            .to_string(),
    )
    .filter(|value| !value.is_empty())
}

fn next_class_name(lines: &[&str], start: usize) -> Option<String> {
    lines.iter().skip(start).take(4).find_map(|line| {
        line.split("class ")
            .nth(1)
            .and_then(|rest| rest.split([' ', '{', '<']).next())
            .map(str::to_string)
    })
}

fn normalized_file(input: &ParseInput) -> String {
    input.path.to_string_lossy().replace('\\', "/")
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
