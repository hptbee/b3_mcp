use b3_core::{
    ArchitectureConfidence, ArchitectureSource, ArchitectureSourceKind, ArchitectureWarning,
};
use b3_storage::StoredInfrastructure;

use super::dependency_keys::{normalize_infra_name, InfraKind, InfraMatchKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfraEntry {
    pub project_id: String,
    pub project_name: String,
    pub key: InfraMatchKey,
    pub record: StoredInfrastructure,
}

impl InfraEntry {
    pub fn source(&self) -> ArchitectureSource {
        ArchitectureSource {
            project_id: self.project_id.clone(),
            file_path: self.record.file_path.clone(),
            symbol_id: Some(self.record.symbol_id.clone()),
            line_start: Some(self.record.line_start),
            line_end: Some(self.record.line_end),
            source_kind: ArchitectureSourceKind::InfrastructureMetadata,
            extractor: Some(self.record.source_kind.clone()),
            metadata_key: Some("infrastructure".to_string()),
        }
    }

    pub fn confidence(&self) -> ArchitectureConfidence {
        ArchitectureConfidence::new(
            b3_core::ArchitectureConfidenceLevel::High,
            self.record.confidence,
            "local infrastructure metadata",
            vec![self.record.source_kind.clone()],
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfraRelationship {
    pub source: InfraEntry,
    pub target: InfraEntry,
    pub relationship: InfraRelationshipKind,
    pub match_rule: String,
    pub confidence: ArchitectureConfidence,
    pub warning: Option<ArchitectureWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfraRelationshipKind {
    DependsOn,
    Selects,
    Deploys,
    References,
    Defines,
}

pub fn collect_infra_entries(
    project_id: &str,
    project_name: &str,
    records: &[StoredInfrastructure],
) -> Vec<InfraEntry> {
    let mut entries = Vec::new();
    for record in records {
        let kind = classify_infra_kind(record);
        let name = record
            .name
            .as_deref()
            .or(record.service_name.as_deref())
            .or(record.image.as_deref())
            .unwrap_or(record.kind.as_str());
        entries.push(InfraEntry {
            project_id: project_id.to_string(),
            project_name: project_name.to_string(),
            key: InfraMatchKey::new(kind, name, record.namespace.as_deref()),
            record: record.clone(),
        });
    }
    entries.sort_by(|left, right| {
        left.key
            .normalized_key
            .cmp(&right.key.normalized_key)
            .then_with(|| left.project_id.cmp(&right.project_id))
            .then_with(|| left.record.file_path.cmp(&right.record.file_path))
    });
    entries
}

pub fn infra_relationships(entries: &[InfraEntry]) -> Vec<InfraRelationship> {
    let mut relationships = Vec::new();
    for source in entries {
        for target in entries {
            if source.record.id == target.record.id && source.project_id == target.project_id {
                continue;
            }
            if let Some((kind, rule, confidence)) = score_infra_relationship(source, target) {
                relationships.push(InfraRelationship {
                    source: source.clone(),
                    target: target.clone(),
                    relationship: kind,
                    match_rule: rule,
                    confidence,
                    warning: None,
                });
            }
        }
    }
    relationships.sort_by(|left, right| {
        right
            .confidence
            .score
            .cmp(&left.confidence.score)
            .then_with(|| left.source.project_id.cmp(&right.source.project_id))
            .then_with(|| left.target.project_id.cmp(&right.target.project_id))
            .then_with(|| {
                left.source
                    .key
                    .normalized_key
                    .cmp(&right.source.key.normalized_key)
            })
            .then_with(|| {
                left.target
                    .key
                    .normalized_key
                    .cmp(&right.target.key.normalized_key)
            })
    });
    relationships.dedup_by(|left, right| {
        left.source.project_id == right.source.project_id
            && left.target.project_id == right.target.project_id
            && left.source.key.normalized_key == right.source.key.normalized_key
            && left.target.key.normalized_key == right.target.key.normalized_key
            && left.relationship == right.relationship
    });
    relationships
}

fn score_infra_relationship(
    source: &InfraEntry,
    target: &InfraEntry,
) -> Option<(InfraRelationshipKind, String, ArchitectureConfidence)> {
    if source.record.technology == "docker_compose"
        && source
            .record
            .selectors
            .iter()
            .map(|value| normalize_infra_name(value))
            .any(|value| value == target.key.name)
    {
        return Some((
            InfraRelationshipKind::DependsOn,
            "compose_depends_on".to_string(),
            ArchitectureConfidence::high("Docker Compose depends_on service relationship"),
        ));
    }

    if source.key.kind == InfraKind::K8sService
        && matches!(
            target.key.kind,
            InfraKind::K8sDeployment | InfraKind::Unknown
        )
        && !source.record.selectors.is_empty()
        && selectors_match_labels(&source.record.selectors, &target.record.labels)
    {
        return Some((
            InfraRelationshipKind::Selects,
            "k8s_selector_labels".to_string(),
            ArchitectureConfidence::high("Kubernetes Service selector matches workload labels"),
        ));
    }

    if matches!(
        source.key.kind,
        InfraKind::K8sDeployment | InfraKind::DockerComposeService
    ) && source
        .record
        .image
        .as_deref()
        .is_some_and(|image| image_matches_name(image, &target.key.name))
    {
        return Some((
            InfraRelationshipKind::Deploys,
            "image_name_overlap".to_string(),
            ArchitectureConfidence::medium("container image overlaps local service/resource name"),
        ));
    }

    if source.key.kind == InfraKind::TerraformModule
        && (source.key.name == target.key.name
            || source.key.name.contains(&target.key.name)
            || target.key.name.contains(&source.key.name))
    {
        return Some((
            InfraRelationshipKind::References,
            "terraform_module_name_overlap".to_string(),
            ArchitectureConfidence::medium(
                "Terraform module/resource name overlaps service/resource",
            ),
        ));
    }

    None
}

fn classify_infra_kind(record: &StoredInfrastructure) -> InfraKind {
    let technology = record.technology.to_ascii_lowercase();
    let kind = record.kind.to_ascii_lowercase();
    let resource_type = record
        .resource_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if technology == "docker_compose" && kind == "service" {
        InfraKind::DockerComposeService
    } else if technology == "docker" && kind == "image" {
        InfraKind::DockerImage
    } else if technology == "kubernetes" && resource_type == "service" {
        InfraKind::K8sService
    } else if technology == "kubernetes" && resource_type == "deployment" {
        InfraKind::K8sDeployment
    } else if technology == "kubernetes" && resource_type == "configmap" {
        InfraKind::K8sConfigMap
    } else if technology == "kubernetes" && resource_type == "secret" {
        InfraKind::K8sSecret
    } else if technology == "terraform" && kind == "module" {
        InfraKind::TerraformModule
    } else if matches!(technology.as_str(), "terraform" | "gcp" | "gke") {
        InfraKind::TerraformResource
    } else if kind.contains("database") || kind == "db" {
        InfraKind::Database
    } else if kind.contains("cache") || kind.contains("redis") {
        InfraKind::Cache
    } else if kind.contains("queue") {
        InfraKind::Queue
    } else {
        InfraKind::Unknown
    }
}

fn selectors_match_labels(selectors: &[String], labels: &[String]) -> bool {
    !selectors.is_empty()
        && selectors
            .iter()
            .all(|selector| labels.iter().any(|label| label == selector))
}

fn image_matches_name(image: &str, name: &str) -> bool {
    let image = image
        .split('/')
        .next_back()
        .unwrap_or(image)
        .split(':')
        .next()
        .unwrap_or(image);
    normalize_infra_name(image) == name
}
