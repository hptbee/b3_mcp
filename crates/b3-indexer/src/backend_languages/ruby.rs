use super::*;

pub(crate) fn parse(input: &ParseInput) -> BackendParseResult {
    if is_project_file(&input.path) {
        return BackendParseResult {
            language: "ruby",
            symbols: project_technology_symbols(
                input,
                "ruby",
                detect_project_technologies(&input.path, &input.source),
            ),
            ..BackendParseResult::default()
        };
    }

    let mut result = BackendParseResult {
        language: "ruby",
        ..BackendParseResult::default()
    };
    let is_routes_file = input
        .path
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with("config/routes.rb");

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(require) = trimmed.strip_prefix("require ") {
            if let Some(value) = literal_in(require) {
                result.symbols.push(BackendSymbol {
                    name: value.clone(),
                    kind: NodeKind::Package,
                    line: line_number,
                    metadata: format!("ruby.require=true;ruby.import_path={value}"),
                });
            }
        }
        if let Some(name) = identifier_after(trimmed, "module ") {
            result.symbols.push(BackendSymbol {
                name: name.to_string(),
                kind: NodeKind::Namespace,
                line: line_number,
                metadata: format!("ruby.module=true;ruby.name={name}"),
            });
        }
        if let Some(name) = identifier_after(trimmed, "class ") {
            result.symbols.push(BackendSymbol {
                name: name.to_string(),
                kind: NodeKind::Class,
                line: line_number,
                metadata: format!("ruby.class=true;ruby.name={name}"),
            });
            if trimmed.contains("< ApplicationRecord") {
                result.data_access.push(BackendDataAccess {
                    technology: "active_record",
                    kind: "Model",
                    operation: None,
                    entity_name: Some(name.to_string()),
                    repository_name: None,
                    query_text: None,
                    class_name: Some(name.to_string()),
                    method_name: None,
                    line: line_number,
                    source_kind: "ActiveRecordModel",
                    confidence: 9_000,
                });
            }
            if trimmed.contains("ApplicationJob") {
                result.messaging.push(BackendMessaging {
                    technology: "active_job",
                    kind: "Consumer",
                    direction: "inbound",
                    topic: None,
                    queue: None,
                    exchange: None,
                    routing_key: None,
                    pattern: None,
                    class_name: Some(name.to_string()),
                    function_name: None,
                    method_name: None,
                    line: line_number,
                    source_kind: "ActiveJobClass",
                    confidence: 7_500,
                });
            }
        }
        if let Some(name) = identifier_after(trimmed, "def ") {
            result.symbols.push(BackendSymbol {
                name: name.to_string(),
                kind: NodeKind::Method,
                line: line_number,
                metadata: format!("ruby.method=true;ruby.name={name}"),
            });
        }
        if let Some((method, path)) = ruby_route(trimmed) {
            result.routes.push(BackendRoute {
                framework: if is_routes_file { "rails" } else { "sinatra" },
                method,
                path,
                handler: route_handler(trimmed),
                class_name: current_class(&result.symbols, line_number),
                function_name: current_function(&result.symbols, line_number),
                line: line_number,
                source_kind: if is_routes_file {
                    "RailsRouteDsl"
                } else {
                    "SinatraRouteLiteral"
                },
                confidence: 8_000,
            });
        }
        if is_routes_file && trimmed.starts_with("resources ") {
            if let Some(resource) =
                symbol_literal_or_name(trimmed.strip_prefix("resources ").unwrap_or_default())
            {
                let path = format!("/{resource}");
                result.routes.push(BackendRoute {
                    framework: "rails",
                    method: "RESOURCE".to_string(),
                    path,
                    handler: None,
                    class_name: None,
                    function_name: None,
                    line: line_number,
                    source_kind: "RailsResourcesRoute",
                    confidence: 7_000,
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
            "ruby",
            "Ruby",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::ExtractSymbols,
            ],
            "Gemfile metadata",
        ));
    }
    for (needle, id, name, kind) in [
        ("rails", "rails", "Rails", TechnologyKind::WebBackend),
        ("sinatra", "sinatra", "Sinatra", TechnologyKind::WebBackend),
        ("sidekiq", "sidekiq", "Sidekiq", TechnologyKind::Messaging),
        (
            "activejob",
            "active_job",
            "ActiveJob",
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
                "Gemfile metadata",
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
        "Gemfile" | "Gemfile.lock"
    )
}

fn ruby_route(trimmed: &str) -> Option<(String, String)> {
    for method in ["get", "post", "put", "patch", "delete"] {
        if trimmed.starts_with(&format!("{method} ")) {
            return Some((method.to_ascii_uppercase(), literal_in(trimmed)?));
        }
    }
    None
}

fn route_handler(trimmed: &str) -> Option<String> {
    trimmed
        .split(" to: ")
        .nth(1)
        .and_then(literal_in)
        .or_else(|| trimmed.split("=>").nth(1).and_then(literal_in))
}

fn symbol_literal_or_name(value: &str) -> Option<String> {
    if let Some(literal) = literal_in(value) {
        return Some(literal.trim_start_matches('/').to_string());
    }
    value
        .trim()
        .trim_start_matches(':')
        .split([',', ' '])
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn collect_data_access(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if trimmed.contains(".where(") || trimmed.contains(".find(") || trimmed.contains(".create(") {
        result.data_access.push(BackendDataAccess {
            technology: "active_record",
            kind: "QueryCall",
            operation: Some(
                if trimmed.contains(".create(") {
                    "create"
                } else {
                    "read"
                }
                .to_string(),
            ),
            entity_name: trimmed.split('.').next().map(str::trim).map(str::to_string),
            repository_name: None,
            query_text: literal_in(trimmed),
            class_name: current_class(&result.symbols, line),
            method_name: current_function(&result.symbols, line),
            line,
            source_kind: "ActiveRecordCall",
            confidence: 8_000,
        });
    }
}

fn collect_messaging(result: &mut BackendParseResult, line: usize, trimmed: &str) {
    if trimmed.contains("Sidekiq::Worker") {
        result.messaging.push(BackendMessaging {
            technology: "sidekiq",
            kind: "Consumer",
            direction: "inbound",
            topic: None,
            queue: None,
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: current_class(&result.symbols, line),
            function_name: None,
            method_name: None,
            line,
            source_kind: "SidekiqWorker",
            confidence: 8_000,
        });
    }
    if trimmed.starts_with("queue_as ") {
        result.messaging.push(BackendMessaging {
            technology: "active_job",
            kind: "Consumer",
            direction: "inbound",
            topic: None,
            queue: symbol_literal_or_name(trimmed.strip_prefix("queue_as ").unwrap_or_default()),
            exchange: None,
            routing_key: None,
            pattern: None,
            class_name: current_class(&result.symbols, line),
            function_name: None,
            method_name: None,
            line,
            source_kind: "ActiveJobQueue",
            confidence: 8_000,
        });
    }
}
