use super::*;

pub(crate) fn parse(input: &ParseInput) -> BackendParseResult {
    if is_project_file(&input.path) {
        return BackendParseResult {
            language: "php",
            symbols: project_technology_symbols(
                input,
                "php",
                detect_project_technologies(&input.path, &input.source),
            ),
            ..BackendParseResult::default()
        };
    }

    let mut result = BackendParseResult {
        language: "php",
        ..BackendParseResult::default()
    };
    let mut pending_symfony_route: Option<(usize, String, String)> = None;

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(namespace) = trimmed
            .strip_prefix("namespace ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            result.symbols.push(BackendSymbol {
                name: namespace.trim().to_string(),
                kind: NodeKind::Namespace,
                line: line_number,
                metadata: format!("php.namespace=true;php.name={}", namespace.trim()),
            });
        }
        if let Some(import) = trimmed
            .strip_prefix("use ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            result.symbols.push(BackendSymbol {
                name: import.trim().to_string(),
                kind: NodeKind::Package,
                line: line_number,
                metadata: format!("php.use=true;php.import_path={}", import.trim()),
            });
        }
        if trimmed.starts_with("#[Route") || trimmed.contains("@Route(") {
            if let Some(path) = literal_in(trimmed) {
                pending_symfony_route = Some((line_number, path, symfony_method(trimmed)));
            }
        }
        if let Some((method, path)) = laravel_route(trimmed).or_else(|| slim_route(trimmed)) {
            result.routes.push(BackendRoute {
                framework: if trimmed.contains("Route::") {
                    "laravel"
                } else {
                    "slim"
                },
                method,
                path,
                handler: literal_controller(trimmed),
                class_name: current_class(&result.symbols, line_number),
                function_name: None,
                line: line_number,
                source_kind: if trimmed.contains("Route::") {
                    "LaravelRouteCall"
                } else {
                    "SlimRouteCall"
                },
                confidence: 8_500,
            });
        }
        if let Some((name, kind)) = parse_type(trimmed) {
            result.symbols.push(BackendSymbol {
                name: name.clone(),
                kind,
                line: line_number,
                metadata: format!("php.type=true;php.name={name}"),
            });
            if trimmed.contains("extends Model") {
                result.data_access.push(BackendDataAccess {
                    technology: "eloquent",
                    kind: "Model",
                    operation: None,
                    entity_name: Some(name.clone()),
                    repository_name: None,
                    query_text: None,
                    class_name: Some(name),
                    method_name: None,
                    line: line_number,
                    source_kind: "EloquentModel",
                    confidence: 9_000,
                });
            }
        }
        if let Some(function_name) = parse_function(trimmed) {
            let kind = if current_class(&result.symbols, line_number).is_some() {
                NodeKind::Method
            } else {
                NodeKind::Function
            };
            result.symbols.push(BackendSymbol {
                name: function_name.clone(),
                kind,
                line: line_number,
                metadata: format!("php.function=true;php.name={function_name}"),
            });
            if let Some((route_line, path, method)) = pending_symfony_route.take() {
                result.routes.push(BackendRoute {
                    framework: "symfony",
                    method,
                    path,
                    handler: Some(function_name.clone()),
                    class_name: current_class(&result.symbols, line_number),
                    function_name: Some(function_name),
                    line: route_line,
                    source_kind: "SymfonyRouteAttribute",
                    confidence: 8_500,
                });
            }
        }
        collect_data_access(&mut result, line_number, trimmed);
        collect_messaging(&mut result, line_number, trimmed);
    }
    result
}

pub(crate) fn detect_project_technologies(path: &Path, source: &str) -> Vec<DetectedTechnology> {
    let mut detected = Vec::new();
    if is_project_file(path) {
        detected.push(technology(
            "php",
            "PHP",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::ExtractSymbols,
            ],
            "composer metadata",
        ));
    }
    for (needle, id, name, kind) in [
        (
            "laravel/framework",
            "laravel",
            "Laravel",
            TechnologyKind::WebBackend,
        ),
        (
            "symfony/framework-bundle",
            "symfony",
            "Symfony",
            TechnologyKind::WebBackend,
        ),
        ("slim/slim", "slim", "Slim", TechnologyKind::WebBackend),
        (
            "doctrine/orm",
            "doctrine",
            "Doctrine ORM",
            TechnologyKind::Orm,
        ),
        (
            "illuminate/queue",
            "laravel_queue",
            "Laravel Queue",
            TechnologyKind::Messaging,
        ),
    ] {
        if detect_dependency(source, &[needle]) {
            detected.push(technology(
                id,
                name,
                kind,
                TechnologySupportLevel::Basic,
                vec![TechnologyCapability::DetectPackage],
                "composer metadata",
            ));
        }
    }
    detected
}

fn is_project_file(path: &Path) -> bool {
    matches!(
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "composer.json" | "composer.lock"
    )
}

fn parse_type(trimmed: &str) -> Option<(String, NodeKind)> {
    for (marker, kind) in [
        ("class ", NodeKind::Class),
        ("interface ", NodeKind::Interface),
        ("trait ", NodeKind::Class),
        ("enum ", NodeKind::Enum),
    ] {
        if let Some(name) = identifier_after(trimmed, marker) {
            return Some((name.to_string(), kind));
        }
    }
    None
}

fn parse_function(trimmed: &str) -> Option<String> {
    identifier_after(trimmed, "function ").map(str::to_string)
}

fn laravel_route(trimmed: &str) -> Option<(String, String)> {
    for method in ["get", "post", "put", "patch", "delete"] {
        let marker = format!("Route::{method}(");
        if trimmed.contains(&marker) {
            return Some((
                method.to_ascii_uppercase(),
                literal_after(trimmed, &marker)?,
            ));
        }
    }
    None
}

fn slim_route(trimmed: &str) -> Option<(String, String)> {
    for method in ["get", "post", "put", "patch", "delete"] {
        let marker = format!("->{method}(");
        if trimmed.contains(&marker) {
            return Some((
                method.to_ascii_uppercase(),
                literal_after(trimmed, &marker)?,
            ));
        }
    }
    None
}

fn symfony_method(trimmed: &str) -> String {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
        if trimmed.contains(method) {
            return method.to_string();
        }
    }
    "ANY".to_string()
}

fn literal_controller(trimmed: &str) -> Option<String> {
    if trimmed.contains("::class") {
        return trimmed
            .split("::class")
            .next()
            .and_then(|left| left.split(['[', ',', ' ']).next_back())
            .map(|value| value.trim_matches(['\\', '\'', '"']).to_string())
            .filter(|value| !value.is_empty());
    }
    literal_in(trimmed)
}

fn collect_data_access(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if trimmed.contains("DB::select") || trimmed.contains("DB::statement") {
        result.data_access.push(BackendDataAccess {
            technology: "raw_sql",
            kind: "QueryCall",
            operation: Some(
                if trimmed.contains("statement") {
                    "execute"
                } else {
                    "read"
                }
                .to_string(),
            ),
            entity_name: None,
            repository_name: None,
            query_text: literal_in(trimmed),
            class_name: current_class(&result.symbols, line),
            method_name: current_function(&result.symbols, line),
            line,
            source_kind: "PhpRawSqlLiteral",
            confidence: 8_000,
        });
    }
}

fn collect_messaging(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if trimmed.contains("ShouldQueue") || trimmed.contains("dispatch(") {
        result.messaging.push(BackendMessaging {
            technology: "laravel_queue",
            kind: if trimmed.contains("dispatch(") {
                "Producer"
            } else {
                "Consumer"
            },
            direction: if trimmed.contains("dispatch(") {
                "outbound"
            } else {
                "inbound"
            },
            topic: None,
            queue: literal_after(trimmed, "onQueue("),
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: current_class(&result.symbols, line),
            function_name: current_function(&result.symbols, line),
            method_name: current_function(&result.symbols, line),
            line,
            source_kind: "LaravelQueueHint",
            confidence: 7_500,
        });
    }
}
