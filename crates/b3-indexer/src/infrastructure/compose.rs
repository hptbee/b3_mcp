use super::*;

#[derive(Debug, Clone)]
struct ComposeService {
    name: String,
    line_start: usize,
    line_end: usize,
    image: Option<String>,
    build: Option<String>,
    ports: Vec<String>,
    env_keys: Vec<String>,
    depends_on: Vec<String>,
}

impl ComposeService {
    fn new(name: String, line_start: usize) -> Self {
        Self {
            name,
            line_start,
            line_end: line_start,
            image: None,
            build: None,
            ports: Vec::new(),
            env_keys: Vec::new(),
            depends_on: Vec::new(),
        }
    }
}

pub(super) fn is_compose_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|value| value.to_str()),
        Some("docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml")
    )
}

pub(super) fn collect_compose(input: &ParseInput) -> Vec<ExtractedSymbol> {
    if !is_compose_file(&input.path) {
        return Vec::new();
    }

    let mut output = Vec::new();
    let mut in_services = false;
    let mut services_indent = 0;
    let mut current: Option<ComposeService> = None;
    let mut active_key: Option<String> = None;

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = leading_spaces(line);
        if trimmed == "services:" {
            in_services = true;
            services_indent = indent;
            continue;
        }
        if !in_services {
            continue;
        }
        if indent <= services_indent && !trimmed.starts_with('-') {
            break;
        }

        if indent == services_indent + 2 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            if let Some(service) = current.take() {
                push_service(input, &mut output, service);
            }
            current = scalar_name(trimmed).map(|name| ComposeService::new(name, line_number));
            active_key = None;
            continue;
        }

        let Some(service) = current.as_mut() else {
            continue;
        };
        service.line_end = line_number;

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = clean_value(value);
            match key {
                "image" => {
                    service.image = Some(value);
                    active_key = Some(key.to_string());
                }
                "build" if !value.is_empty() => {
                    service.build = Some(value);
                    active_key = Some(key.to_string());
                }
                "ports" | "environment" | "depends_on" => active_key = Some(key.to_string()),
                key if active_key.as_deref() == Some("environment") => {
                    if let Some(env_key) = env_key(key) {
                        service.env_keys.push(env_key);
                    }
                }
                _ => active_key = Some(key.to_string()),
            }
            continue;
        }

        if let Some(value) = list_value(trimmed) {
            match active_key.as_deref() {
                Some("ports") => service.ports.push(value),
                Some("environment") => {
                    if let Some(env_key) = env_key(&value) {
                        service.env_keys.push(env_key);
                    }
                }
                Some("depends_on") => service.depends_on.push(value),
                _ => {}
            }
        }
    }

    if let Some(service) = current {
        push_service(input, &mut output, service);
    }

    output
}

fn push_service(input: &ParseInput, output: &mut Vec<ExtractedSymbol>, service: ComposeService) {
    let mut metadata = metadata(
        input,
        service.line_start,
        service.line_end,
        "docker_compose",
        "Service",
        "ComposeService",
    );
    metadata.name = Some(service.name.clone());
    metadata.service_name = Some(service.name.clone());
    metadata.image = service.image.or(service.build);
    metadata.ports = service.ports;
    metadata.env_keys = service.env_keys;
    metadata.selectors = service.depends_on;
    metadata.confidence = 9_000;
    output.push(infrastructure_symbol(
        input,
        service.line_start,
        &format!("Compose service {}", service.name),
        metadata,
    ));
}
