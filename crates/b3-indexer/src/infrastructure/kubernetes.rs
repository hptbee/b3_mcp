use super::*;

#[derive(Debug, Clone)]
struct KubernetesDoc {
    line_start: usize,
    line_end: usize,
    kind: Option<String>,
    name: Option<String>,
    namespace: Option<String>,
    labels: Vec<String>,
    selectors: Vec<String>,
    containers: Vec<String>,
    images: Vec<String>,
    ports: Vec<String>,
    ingress_backends: Vec<String>,
}

impl KubernetesDoc {
    fn new(line_start: usize) -> Self {
        Self {
            line_start,
            line_end: line_start,
            kind: None,
            name: None,
            namespace: None,
            labels: Vec::new(),
            selectors: Vec::new(),
            containers: Vec::new(),
            images: Vec::new(),
            ports: Vec::new(),
            ingress_backends: Vec::new(),
        }
    }
}

pub(super) fn is_kubernetes_yaml(path: &Path, source: &str) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    if !matches!(extension.to_ascii_lowercase().as_str(), "yaml" | "yml") {
        return false;
    }
    source.contains("apiVersion:")
        && source.contains("kind:")
        && source.contains("metadata:")
        && kubernetes_kind(source).is_some()
}

pub(super) fn collect_kubernetes(input: &ParseInput) -> Vec<ExtractedSymbol> {
    if !is_kubernetes_yaml(&input.path, &input.source) {
        return Vec::new();
    }

    let mut output = Vec::new();
    let mut doc = KubernetesDoc::new(1);
    let mut section_stack: Vec<(usize, String)> = Vec::new();
    let mut in_metadata = false;

    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "---" {
            push_doc(input, &mut output, doc);
            doc = KubernetesDoc::new(line_number + 1);
            section_stack.clear();
            in_metadata = false;
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        doc.line_end = line_number;
        let indent = leading_spaces(line);
        while section_stack
            .last()
            .is_some_and(|(level, _)| *level >= indent)
        {
            section_stack.pop();
        }

        if trimmed.ends_with(':') && !trimmed.starts_with('-') {
            if let Some(name) = scalar_name(trimmed) {
                in_metadata = name == "metadata";
                section_stack.push((indent, name));
            }
            continue;
        }

        if let Some(value) = value_after_colon(trimmed) {
            let key = trimmed
                .split_once(':')
                .map(|(key, _)| key.trim().trim_start_matches("- ").trim());
            match key {
                Some("kind") => doc.kind = Some(value),
                Some("name") if in_metadata => doc.name = Some(value),
                Some("namespace") if in_metadata => doc.namespace = Some(value),
                Some("name") if in_section(&section_stack, "containers") => {
                    doc.containers.push(value);
                }
                Some("image") => doc.images.push(value),
                Some("containerPort" | "port" | "targetPort") => doc.ports.push(value),
                Some("serviceName") | Some("service") if doc.kind.as_deref() == Some("Ingress") => {
                    doc.ingress_backends.push(value);
                }
                Some("name")
                    if doc.kind.as_deref() == Some("Ingress")
                        && in_section(&section_stack, "service") =>
                {
                    doc.ingress_backends.push(value);
                }
                Some(key) if in_section(&section_stack, "labels") => {
                    doc.labels.push(format!("{key}={value}"));
                }
                Some(key)
                    if in_section(&section_stack, "selector")
                        || in_section(&section_stack, "matchLabels") =>
                {
                    doc.selectors.push(format!("{key}={value}"));
                }
                Some(key) if key == "iam.gke.io/gcp-service-account" => {
                    doc.labels.push(format!("{key}={value}"));
                }
                _ => {}
            }
        }
    }

    push_doc(input, &mut output, doc);
    output
}

fn push_doc(input: &ParseInput, output: &mut Vec<ExtractedSymbol>, doc: KubernetesDoc) {
    let Some(kind) = doc.kind else {
        return;
    };
    if !is_common_kind(&kind) {
        return;
    }

    let (technology, source_kind) = if doc
        .labels
        .iter()
        .any(|label| label.starts_with("iam.gke.io/gcp-service-account="))
    {
        ("gke", "GkeKubernetesManifest")
    } else {
        ("kubernetes", source_kind_for_kind(&kind))
    };
    let mut metadata = metadata(
        input,
        doc.line_start,
        doc.line_end,
        technology,
        &kind,
        source_kind,
    );
    metadata.name = doc.name.clone();
    metadata.resource_type = Some(kind.clone());
    metadata.namespace = doc.namespace;
    metadata.container_name = doc.containers.first().cloned();
    metadata.image = doc.images.first().cloned();
    metadata.ports = doc.ports;
    metadata.labels = doc.labels;
    metadata.selectors = if kind == "Ingress" && !doc.ingress_backends.is_empty() {
        doc.ingress_backends
    } else {
        doc.selectors
    };
    metadata.confidence = 9_000;

    output.push(infrastructure_symbol(
        input,
        doc.line_start,
        &format!("Kubernetes {kind}"),
        metadata,
    ));
}

fn kubernetes_kind(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with("kind:") {
            value_after_colon(trimmed)
        } else {
            None
        }
    })
}

fn in_section(stack: &[(usize, String)], section: &str) -> bool {
    stack.iter().any(|(_, value)| value == section)
}

fn is_common_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Deployment"
            | "StatefulSet"
            | "DaemonSet"
            | "Job"
            | "CronJob"
            | "Service"
            | "Ingress"
            | "ConfigMap"
            | "Secret"
            | "Namespace"
            | "ServiceAccount"
            | "HorizontalPodAutoscaler"
    )
}

fn source_kind_for_kind(kind: &str) -> &'static str {
    match kind {
        "Deployment" => "KubernetesDeployment",
        "Service" => "KubernetesService",
        "Ingress" => "KubernetesIngress",
        "ConfigMap" => "KubernetesConfigMap",
        "Secret" => "KubernetesSecret",
        "Namespace" => "KubernetesNamespace",
        "ServiceAccount" => "KubernetesServiceAccount",
        _ => "KubernetesManifest",
    }
}
