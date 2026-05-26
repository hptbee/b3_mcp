use super::*;

pub(crate) fn parse(input: &ParseInput) -> BackendParseResult {
    if is_project_file(&input.path) {
        return BackendParseResult {
            language: "kotlin",
            symbols: project_technology_symbols(
                input,
                "kotlin",
                detect_project_technologies(&input.path, &input.source),
            ),
            ..BackendParseResult::default()
        };
    }

    let mut result = BackendParseResult {
        language: "kotlin",
        ..BackendParseResult::default()
    };
    let mut class_prefix = String::new();
    let mut pending_route: Option<(usize, String, String, &'static str)> = None;
    let mut route_stack: Vec<String> = Vec::new();

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(package) = trimmed.strip_prefix("package ") {
            result.symbols.push(BackendSymbol {
                name: package.trim().to_string(),
                kind: NodeKind::Namespace,
                line: line_number,
                metadata: format!("kotlin.package=true;kotlin.name={}", package.trim()),
            });
        }
        if let Some(import) = trimmed.strip_prefix("import ") {
            result.symbols.push(BackendSymbol {
                name: import.trim().to_string(),
                kind: NodeKind::Package,
                line: line_number,
                metadata: format!("kotlin.import=true;kotlin.import_path={}", import.trim()),
            });
        }
        if trimmed.starts_with("@RequestMapping") {
            class_prefix = annotation_literal(trimmed, "@RequestMapping").unwrap_or_default();
            continue;
        }
        if let Some((method, path)) = route_method_from_annotation(trimmed) {
            pending_route = Some((
                line_number,
                method.to_string(),
                path,
                "KotlinSpringRouteAnnotation",
            ));
            continue;
        }
        if let Some((method, path)) = ktor_route(trimmed) {
            let prefix = route_stack.last().map(String::as_str).unwrap_or("");
            result.routes.push(BackendRoute {
                framework: "ktor",
                method,
                path: normalize_route_path(prefix, &path),
                handler: None,
                class_name: current_class(&result.symbols, line_number),
                function_name: current_function(&result.symbols, line_number),
                line: line_number,
                source_kind: "KtorRouteLiteral",
                confidence: 8_000,
            });
        }
        if trimmed.starts_with("route(") {
            if let Some(path) = literal_in(trimmed) {
                route_stack.push(path);
            }
        }
        if trimmed == "}" {
            route_stack.pop();
        }

        if let Some((name, kind)) = parse_type(trimmed) {
            result.symbols.push(BackendSymbol {
                name: name.clone(),
                kind,
                line: line_number,
                metadata: format!("kotlin.type=true;kotlin.name={name}"),
            });
            if has_recent_annotation(input, line_number, "@Entity") {
                result.data_access.push(BackendDataAccess {
                    technology: "jpa",
                    kind: "Entity",
                    operation: None,
                    entity_name: Some(name.clone()),
                    repository_name: None,
                    query_text: None,
                    class_name: Some(name),
                    method_name: None,
                    line: line_number,
                    source_kind: "KotlinJpaEntity",
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
                metadata: format!("kotlin.function=true;kotlin.name={function_name}"),
            });
            if let Some((route_line, method, path, source_kind)) = pending_route.take() {
                result.routes.push(BackendRoute {
                    framework: "spring",
                    method,
                    path: normalize_route_path(&class_prefix, &path),
                    handler: Some(function_name.clone()),
                    class_name: current_class(&result.symbols, line_number),
                    function_name: Some(function_name),
                    line: route_line,
                    source_kind,
                    confidence: 9_000,
                });
            }
        }
        collect_messaging(&mut result, line_number, trimmed);
    }
    result
}

pub(crate) fn detect_project_technologies(path: &Path, source: &str) -> Vec<DetectedTechnology> {
    let mut detected = Vec::new();
    if is_project_file(path) || path.extension().and_then(|value| value.to_str()) == Some("kt") {
        detected.push(technology(
            "kotlin",
            "Kotlin",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::ExtractSymbols,
            ],
            "kotlin project metadata",
        ));
    }
    for (needle, id, name, kind) in [
        (
            "spring-boot",
            "spring",
            "Spring",
            TechnologyKind::WebBackend,
        ),
        ("ktor", "ktor", "Ktor", TechnologyKind::WebBackend),
        ("spring-kafka", "kafka", "Kafka", TechnologyKind::Messaging),
        (
            "spring-rabbit",
            "rabbitmq",
            "RabbitMQ",
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
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("kotlin project"),
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
        "build.gradle.kts" | "settings.gradle.kts"
    )
}

fn parse_type(trimmed: &str) -> Option<(String, NodeKind)> {
    for (marker, kind) in [
        ("data class ", NodeKind::Class),
        ("class ", NodeKind::Class),
        ("object ", NodeKind::Class),
        ("interface ", NodeKind::Interface),
        ("enum class ", NodeKind::Enum),
    ] {
        if let Some(name) = identifier_after(trimmed, marker) {
            return Some((name.to_string(), kind));
        }
    }
    None
}

fn parse_function(trimmed: &str) -> Option<String> {
    identifier_after(trimmed, "fun ").map(str::to_string)
}

fn ktor_route(trimmed: &str) -> Option<(String, String)> {
    for method in ["get", "post", "put", "patch", "delete"] {
        if trimmed.starts_with(&format!("{method}(")) {
            return Some((method.to_ascii_uppercase(), literal_in(trimmed)?));
        }
    }
    None
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
            source_kind: "KotlinKafkaListener",
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
            source_kind: "KotlinRabbitListener",
            confidence: 8_500,
        });
    }
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
