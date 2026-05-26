use super::*;

pub(crate) fn parse(input: &ParseInput) -> BackendParseResult {
    if is_project_file(&input.path) {
        return parse_project(input);
    }

    let mut result = BackendParseResult {
        language: "python",
        ..BackendParseResult::default()
    };
    let mut pending_decorators: Vec<(usize, String)> = Vec::new();
    let mut router_prefixes: Vec<(String, String)> = Vec::new();

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(import) = parse_import(trimmed) {
            result.symbols.push(BackendSymbol {
                name: import.clone(),
                kind: NodeKind::Package,
                line: line_number,
                metadata: format!("python.import=true;python.import_path={import}"),
            });
        }

        if let Some((router, prefix)) = parse_router_prefix(trimmed) {
            router_prefixes.push((router, prefix));
        }

        if trimmed.starts_with('@') {
            pending_decorators.push((line_number, trimmed.to_string()));
            collect_route_decorator(&mut result, &router_prefixes, line_number, trimmed, None);
            collect_messaging_decorator(&mut result, line_number, trimmed, None);
            continue;
        }

        if let Some(name) = identifier_after(trimmed, "class ") {
            let is_model = trimmed.contains("models.Model")
                || pending_decorators.iter().any(|(_, value)| {
                    value.contains("declarative") || value.contains("@dataclass")
                });
            result.symbols.push(BackendSymbol {
                name: name.to_string(),
                kind: NodeKind::Class,
                line: line_number,
                metadata: format!("python.class=true;python.name={name}"),
            });
            if is_model {
                result.data_access.push(BackendDataAccess {
                    technology: if trimmed.contains("models.Model") {
                        "django_orm"
                    } else {
                        "sqlalchemy"
                    },
                    kind: "Model",
                    operation: None,
                    entity_name: Some(name.to_string()),
                    repository_name: None,
                    query_text: None,
                    class_name: Some(name.to_string()),
                    method_name: None,
                    line: line_number,
                    source_kind: if trimmed.contains("models.Model") {
                        "DjangoModel"
                    } else {
                        "SqlAlchemyModel"
                    },
                    confidence: 9_000,
                });
            }
            pending_decorators.clear();
            continue;
        }

        if let Some((name, is_async)) = parse_function(trimmed) {
            let kind = if line.starts_with("    ") || line.starts_with('\t') {
                NodeKind::Method
            } else {
                NodeKind::Function
            };
            result.symbols.push(BackendSymbol {
                name: name.clone(),
                kind,
                line: line_number,
                metadata: format!(
                    "python.function=true;python.name={name};python.async={}",
                    if is_async { "true" } else { "false" }
                ),
            });
            for (decorator_line, decorator) in &pending_decorators {
                collect_route_decorator(
                    &mut result,
                    &router_prefixes,
                    *decorator_line,
                    decorator,
                    Some(name.clone()),
                );
                collect_messaging_decorator(
                    &mut result,
                    *decorator_line,
                    decorator,
                    Some(name.clone()),
                );
            }
            pending_decorators.clear();
            continue;
        }

        collect_django_url(&mut result, line_number, trimmed);
        let symbol_context = result.symbols.clone();
        collect_data_access_call(&mut result, &symbol_context, line_number, trimmed);
        collect_messaging_call(&mut result, &symbol_context, line_number, trimmed);
        if is_constant_assignment(trimmed) {
            let name = trimmed.split('=').next().unwrap_or_default().trim();
            result.symbols.push(BackendSymbol {
                name: name.to_string(),
                kind: NodeKind::Variable,
                line: line_number,
                metadata: format!("python.constant=true;python.name={name}"),
            });
        }
    }

    result
}

pub(crate) fn detect_project_technologies(path: &Path, source: &str) -> Vec<DetectedTechnology> {
    let mut detected = Vec::new();
    if is_project_file(path) {
        detected.push(language_technology(
            "python",
            "Python",
            "python project metadata",
        ));
    }
    for (needle, id, name, kind) in [
        ("fastapi", "fastapi", "FastAPI", TechnologyKind::WebBackend),
        ("flask", "flask", "Flask", TechnologyKind::WebBackend),
        ("django", "django", "Django", TechnologyKind::WebBackend),
        (
            "sqlalchemy",
            "sqlalchemy",
            "SQLAlchemy",
            TechnologyKind::Orm,
        ),
        ("celery", "celery", "Celery", TechnologyKind::Messaging),
        ("pika", "pika", "Pika/RabbitMQ", TechnologyKind::Messaging),
        ("kafka", "kafka", "Kafka", TechnologyKind::Messaging),
    ] {
        if detect_dependency(source, &[needle]) {
            detected.push(technology(
                id,
                name,
                kind,
                TechnologySupportLevel::Basic,
                vec![
                    TechnologyCapability::DetectPackage,
                    TechnologyCapability::ExtractSymbols,
                ],
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("python project"),
            ));
        }
    }
    detected
}

fn parse_project(input: &ParseInput) -> BackendParseResult {
    BackendParseResult {
        language: "python",
        symbols: project_technology_symbols(
            input,
            "python",
            detect_project_technologies(&input.path, &input.source),
        ),
        ..BackendParseResult::default()
    }
}

fn is_project_file(path: &Path) -> bool {
    matches!(
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "pyproject.toml" | "requirements.txt" | "setup.cfg" | "Pipfile" | "poetry.lock" | "uv.lock"
    )
}

fn parse_import(trimmed: &str) -> Option<String> {
    if let Some(rest) = trimmed.strip_prefix("import ") {
        return rest
            .split([',', ' '])
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
    if let Some(rest) = trimmed.strip_prefix("from ") {
        return rest
            .split(" import ")
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
    None
}

fn parse_function(trimmed: &str) -> Option<(String, bool)> {
    let (rest, is_async) = trimmed
        .strip_prefix("async def ")
        .map(|rest| (rest, true))
        .or_else(|| trimmed.strip_prefix("def ").map(|rest| (rest, false)))?;
    Some((rest.split('(').next()?.trim().to_string(), is_async))
        .filter(|(name, _)| !name.is_empty())
}

fn parse_router_prefix(trimmed: &str) -> Option<(String, String)> {
    if !trimmed.contains("APIRouter(") || !trimmed.contains('=') {
        return None;
    }
    let variable = trimmed.split('=').next()?.trim().to_string();
    let prefix = literal_after(trimmed, "prefix=").unwrap_or_default();
    Some((variable, prefix))
}

fn collect_route_decorator(
    result: &mut BackendParseResult,
    prefixes: &[(String, String)],
    line: usize,
    decorator: &str,
    handler: Option<String>,
) {
    let Some((target, method)) = decorator
        .strip_prefix('@')
        .and_then(|value| value.split_once('.'))
        .and_then(|(target, rest)| {
            let method = rest.split('(').next()?.to_ascii_uppercase();
            matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE")
                .then_some((target, method))
        })
    else {
        if decorator.starts_with("@app.route") || decorator.contains(".route(") {
            if let Some(path) = literal_in(decorator) {
                result.routes.push(BackendRoute {
                    framework: "flask",
                    method: flask_method(decorator),
                    path,
                    handler: handler.clone(),
                    class_name: None,
                    function_name: handler,
                    line,
                    source_kind: "FlaskRouteDecorator",
                    confidence: 8_500,
                });
            }
        }
        return;
    };
    let Some(path) = literal_in(decorator) else {
        return;
    };
    let prefix = prefixes
        .iter()
        .find(|(name, _)| name == target)
        .map(|(_, prefix)| prefix.as_str())
        .unwrap_or("");
    result.routes.push(BackendRoute {
        framework: "fastapi",
        method,
        path: normalize_route_path(prefix, &path),
        handler: handler.clone(),
        class_name: None,
        function_name: handler,
        line,
        source_kind: "FastApiRouteDecorator",
        confidence: if prefix.is_empty() { 8_500 } else { 9_000 },
    });
}

fn flask_method(decorator: &str) -> String {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
        if decorator.contains(method) {
            return method.to_string();
        }
    }
    "GET".to_string()
}

fn collect_django_url(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if !(trimmed.contains("path(") || trimmed.contains("re_path(")) {
        return;
    }
    let Some(path) = literal_in(trimmed) else {
        return;
    };
    result.routes.push(BackendRoute {
        framework: "django",
        method: "ANY".to_string(),
        path,
        handler: None,
        class_name: None,
        function_name: None,
        line,
        source_kind: "DjangoUrlPattern",
        confidence: 8_000,
    });
}

fn collect_data_access_call(
    result: &mut BackendParseResult,
    symbols: &[BackendSymbol],
    line: usize,
    trimmed: &str,
) {
    let (technology, source_kind, operation) = if trimmed.contains(".query(")
        || trimmed.contains("select(")
    {
        ("sqlalchemy", "SqlAlchemyQuery", "read")
    } else if trimmed.contains(".objects.filter(") || trimmed.contains(".objects.get(") {
        ("django_orm", "DjangoOrmRead", "read")
    } else if trimmed.contains(".objects.create(") {
        ("django_orm", "DjangoOrmCreate", "create")
    } else if trimmed.to_ascii_lowercase().contains("select ") && literal_in(trimmed).is_some() {
        ("raw_sql", "PythonRawSqlLiteral", "read")
    } else {
        return;
    };
    result.data_access.push(BackendDataAccess {
        technology,
        kind: "QueryCall",
        operation: Some(operation.to_string()),
        entity_name: receiver_before(trimmed, ".objects."),
        repository_name: None,
        query_text: literal_in(trimmed),
        class_name: current_class(symbols, line),
        method_name: current_function(symbols, line),
        line,
        source_kind,
        confidence: 8_000,
    });
}

fn collect_messaging_decorator(
    result: &mut BackendParseResult,
    line: usize,
    decorator: &str,
    handler: Option<String>,
) {
    if decorator.contains(".task") || decorator.contains("@shared_task") {
        result.messaging.push(BackendMessaging {
            technology: "celery",
            kind: "Consumer",
            direction: "inbound",
            topic: literal_after(decorator, "name="),
            queue: literal_after(decorator, "queue="),
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: None,
            function_name: handler.clone(),
            method_name: handler,
            line,
            source_kind: "CeleryTaskDecorator",
            confidence: 8_000,
        });
    }
}

fn collect_messaging_call(
    result: &mut BackendParseResult,
    symbols: &[BackendSymbol],
    line: usize,
    trimmed: &str,
) {
    if trimmed.contains("basic_publish(") {
        result.messaging.push(BackendMessaging {
            technology: "rabbitmq",
            kind: "Producer",
            direction: "outbound",
            topic: None,
            queue: None,
            exchange: literal_after(trimmed, "exchange="),
            routing_key: literal_after(trimmed, "routing_key="),
            pattern: None,
            class_name: current_class(symbols, line),
            function_name: current_function(symbols, line),
            method_name: current_function(symbols, line),
            line,
            source_kind: "PikaBasicPublish",
            confidence: 8_500,
        });
    }
    if trimmed.contains("basic_consume(") {
        result.messaging.push(BackendMessaging {
            technology: "rabbitmq",
            kind: "Consumer",
            direction: "inbound",
            topic: None,
            queue: literal_after(trimmed, "queue="),
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: current_class(symbols, line),
            function_name: current_function(symbols, line),
            method_name: current_function(symbols, line),
            line,
            source_kind: "PikaBasicConsume",
            confidence: 8_500,
        });
    }
}

fn is_constant_assignment(trimmed: &str) -> bool {
    trimmed
        .split('=')
        .next()
        .is_some_and(|left| left.chars().all(|ch| ch == '_' || ch.is_ascii_uppercase()))
        && trimmed.contains('=')
}

fn receiver_before(trimmed: &str, marker: &str) -> Option<String> {
    let pos = trimmed.find(marker)?;
    trimmed[..pos]
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next_back()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn language_technology(id: &str, name: &str, source: &str) -> DetectedTechnology {
    technology(
        id,
        name,
        TechnologyKind::Language,
        TechnologySupportLevel::Basic,
        vec![
            TechnologyCapability::DetectPackage,
            TechnologyCapability::DetectImport,
            TechnologyCapability::ExtractSymbols,
            TechnologyCapability::ExtractRoutes,
        ],
        source,
    )
}
