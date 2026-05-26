use super::*;

pub(crate) fn parse(input: &ParseInput) -> BackendParseResult {
    if is_project_file(&input.path) {
        return parse_project(input);
    }

    let mut result = BackendParseResult {
        language: "java",
        ..BackendParseResult::default()
    };
    let mut class_prefix = String::new();
    let mut pending_route: Option<(usize, &'static str, String, &'static str)> = None;
    let mut pending_jaxrs_method: Option<&'static str> = None;
    let mut pending_jaxrs_path: Option<String> = None;

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if let Some(package) = trimmed
            .strip_prefix("package ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            result.symbols.push(BackendSymbol {
                name: package.trim().to_string(),
                kind: NodeKind::Namespace,
                line: line_number,
                metadata: format!("java.package=true;java.name={}", package.trim()),
            });
        }
        if let Some(import) = trimmed
            .strip_prefix("import ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            result.symbols.push(BackendSymbol {
                name: import.trim().to_string(),
                kind: NodeKind::Package,
                line: line_number,
                metadata: format!("java.import=true;java.import_path={}", import.trim()),
            });
        }

        if trimmed.starts_with("@RequestMapping") {
            let path = annotation_literal(trimmed, "@RequestMapping").unwrap_or_default();
            if trimmed.contains("method") {
                pending_route = Some((
                    line_number,
                    spring_method(trimmed),
                    path,
                    "SpringRequestMapping",
                ));
            } else {
                class_prefix = path;
            }
            continue;
        }
        if let Some((method, path)) = route_method_from_annotation(trimmed) {
            if trimmed.starts_with("@GET")
                || trimmed.starts_with("@POST")
                || trimmed.starts_with("@PUT")
                || trimmed.starts_with("@PATCH")
                || trimmed.starts_with("@DELETE")
            {
                pending_jaxrs_method = Some(method);
            } else {
                pending_route = Some((line_number, method, path, "SpringRouteAnnotation"));
            }
            continue;
        }
        if trimmed.starts_with("@Path") {
            pending_jaxrs_path = annotation_literal(trimmed, "@Path");
            continue;
        }

        if let Some((name, kind)) = parse_type(trimmed) {
            result.symbols.push(BackendSymbol {
                name: name.clone(),
                kind,
                line: line_number,
                metadata: format!("java.type=true;java.name={name}"),
            });
            if has_recent_annotation(input, line_number, "@Entity") {
                result.data_access.push(BackendDataAccess {
                    technology: "jpa",
                    kind: "Entity",
                    operation: None,
                    entity_name: Some(name.clone()),
                    repository_name: None,
                    query_text: None,
                    class_name: Some(name.clone()),
                    method_name: None,
                    line: line_number,
                    source_kind: "JpaEntity",
                    confidence: 9_000,
                });
            }
            if trimmed.contains("extends JpaRepository")
                || trimmed.contains("extends CrudRepository")
            {
                result.data_access.push(BackendDataAccess {
                    technology: "jpa",
                    kind: "Repository",
                    operation: None,
                    entity_name: generic_first(trimmed),
                    repository_name: Some(name.clone()),
                    query_text: None,
                    class_name: Some(name),
                    method_name: None,
                    line: line_number,
                    source_kind: "SpringRepositoryInterface",
                    confidence: 9_000,
                });
            }
            continue;
        }

        if let Some(method_name) = parse_method(trimmed) {
            result.symbols.push(BackendSymbol {
                name: method_name.clone(),
                kind: NodeKind::Method,
                line: line_number,
                metadata: format!("java.method=true;java.name={method_name}"),
            });
            if let Some((route_line, method, path, source_kind)) = pending_route.take() {
                result.routes.push(BackendRoute {
                    framework: "spring",
                    method: method.to_string(),
                    path: normalize_route_path(&class_prefix, &path),
                    handler: Some(method_name.clone()),
                    class_name: current_class(&result.symbols, line_number),
                    function_name: Some(method_name.clone()),
                    line: route_line,
                    source_kind,
                    confidence: 9_000,
                });
            }
            if let (Some(method), Some(path)) =
                (pending_jaxrs_method.take(), pending_jaxrs_path.take())
            {
                result.routes.push(BackendRoute {
                    framework: "jaxrs",
                    method: method.to_string(),
                    path,
                    handler: Some(method_name.clone()),
                    class_name: current_class(&result.symbols, line_number),
                    function_name: Some(method_name),
                    line: line_number,
                    source_kind: "JaxRsRouteAnnotation",
                    confidence: 8_500,
                });
            }
        }

        collect_messaging(&mut result, line_number, trimmed);
        collect_data_access(&mut result, line_number, trimmed);
    }

    result
}

pub(crate) fn detect_project_technologies(path: &Path, source: &str) -> Vec<DetectedTechnology> {
    let mut detected = Vec::new();
    if is_project_file(path) {
        detected.push(technology(
            "java",
            "Java",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::ExtractSymbols,
            ],
            "java project metadata",
        ));
    }
    for (needle, id, name, kind) in [
        (
            "spring-boot",
            "spring",
            "Spring",
            TechnologyKind::WebBackend,
        ),
        (
            "jakarta.ws.rs",
            "jaxrs",
            "JAX-RS",
            TechnologyKind::WebBackend,
        ),
        ("javax.ws.rs", "jaxrs", "JAX-RS", TechnologyKind::WebBackend),
        (
            "micronaut",
            "micronaut",
            "Micronaut",
            TechnologyKind::WebBackend,
        ),
        ("quarkus", "quarkus", "Quarkus", TechnologyKind::WebBackend),
        ("spring-kafka", "kafka", "Kafka", TechnologyKind::Messaging),
        (
            "spring-rabbit",
            "rabbitmq",
            "RabbitMQ",
            TechnologyKind::Messaging,
        ),
        ("hibernate", "jpa", "JPA/Hibernate", TechnologyKind::Orm),
    ] {
        if detect_dependency(source, &[needle]) {
            detected.push(technology(
                id,
                name,
                kind,
                TechnologySupportLevel::Basic,
                vec![TechnologyCapability::DetectPackage],
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("java project"),
            ));
        }
    }
    detected
}

fn parse_project(input: &ParseInput) -> BackendParseResult {
    BackendParseResult {
        language: "java",
        symbols: project_technology_symbols(
            input,
            "java",
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
        "pom.xml" | "build.gradle" | "settings.gradle"
    )
}

fn parse_type(trimmed: &str) -> Option<(String, NodeKind)> {
    for (marker, kind) in [
        (" class ", NodeKind::Class),
        (" interface ", NodeKind::Interface),
        (" enum ", NodeKind::Enum),
        (" record ", NodeKind::Struct),
    ] {
        if let Some(name) = identifier_after(&format!(" {trimmed}"), marker) {
            return Some((name.to_string(), kind));
        }
    }
    None
}

fn parse_method(trimmed: &str) -> Option<String> {
    if !trimmed.contains('(') || trimmed.starts_with('@') || trimmed.contains(" class ") {
        return None;
    }
    let before = trimmed.split('(').next()?.trim();
    let name = before.split_whitespace().next_back()?;
    (!matches!(name, "if" | "for" | "while" | "switch" | "catch")).then(|| name.to_string())
}

fn spring_method(trimmed: &str) -> &'static str {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
        if trimmed.contains(method) {
            return method;
        }
    }
    "ANY"
}

fn collect_messaging(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if trimmed.starts_with("@KafkaListener") {
        result.messaging.push(BackendMessaging {
            technology: "kafka",
            kind: "Consumer",
            direction: "inbound",
            topic: literal_after(trimmed, "topics").or_else(|| literal_after(trimmed, "topic")),
            queue: None,
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: current_class(&result.symbols, line),
            function_name: None,
            method_name: None,
            line,
            source_kind: "KafkaListener",
            confidence: 8_500,
        });
    }
    if trimmed.starts_with("@RabbitListener") {
        result.messaging.push(BackendMessaging {
            technology: "rabbitmq",
            kind: "Consumer",
            direction: "inbound",
            topic: None,
            queue: literal_after(trimmed, "queues").or_else(|| literal_after(trimmed, "queue")),
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: current_class(&result.symbols, line),
            function_name: None,
            method_name: None,
            line,
            source_kind: "RabbitListener",
            confidence: 8_500,
        });
    }
}

fn collect_data_access(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if trimmed.to_ascii_lowercase().contains("select ") && literal_in(trimmed).is_some() {
        result.data_access.push(BackendDataAccess {
            technology: "jdbc",
            kind: "QueryCall",
            operation: Some("read".to_string()),
            entity_name: None,
            repository_name: None,
            query_text: literal_in(trimmed),
            class_name: current_class(&result.symbols, line),
            method_name: current_function(&result.symbols, line),
            line,
            source_kind: "JdbcRawSqlLiteral",
            confidence: 8_000,
        });
    }
}

fn generic_first(trimmed: &str) -> Option<String> {
    trimmed
        .split('<')
        .nth(1)?
        .split([',', '>'])
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn has_recent_annotation(input: &ParseInput, line: usize, annotation: &str) -> bool {
    let lines: Vec<&str> = input.source.lines().collect();
    lines
        .iter()
        .take(line.saturating_sub(1))
        .rev()
        .take(4)
        .any(|value| value.trim().starts_with(annotation))
}
