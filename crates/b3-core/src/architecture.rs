//! Cross-project architecture contracts.
//!
//! Phase 11.0 defines portable DTOs and deterministic normalization helpers
//! only. It does not federate project databases or match routes/messages.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::stable_hash;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIdentity {
    pub project_id: String,
    pub display_name: String,
    pub root_path: String,
    pub database_path: String,
    pub default_branch: Option<String>,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub service_hints: Vec<String>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub warnings: Vec<ArchitectureWarning>,
}

impl ProjectIdentity {
    pub fn new(
        project_id: impl Into<String>,
        display_name: impl Into<String>,
        root_path: impl Into<String>,
        database_path: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            display_name: display_name.into(),
            root_path: root_path.into(),
            database_path: database_path.into(),
            default_branch: None,
            languages: Vec::new(),
            frameworks: Vec::new(),
            service_hints: Vec::new(),
            tags: Vec::new(),
            metadata: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_missing_database_warning(mut self) -> Self {
        self.warnings.push(ArchitectureWarning::missing_database(
            self.project_id.clone(),
            self.database_path.clone(),
        ));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectGroupIdentity {
    pub group_id: String,
    pub name: String,
    pub project_ids: Vec<String>,
    pub created_from_registry: bool,
    pub metadata: BTreeMap<String, String>,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceIdentity {
    pub service_id: String,
    pub project_id: String,
    pub name: String,
    pub kind: ServiceKind,
    pub root_path: Option<String>,
    pub language_hints: Vec<String>,
    pub framework_hints: Vec<String>,
    pub entrypoint_hints: Vec<String>,
    pub infra_hints: Vec<String>,
    pub package_name: Option<String>,
    pub docker_service_name: Option<String>,
    pub compose_service_name: Option<String>,
    pub kubernetes_service_name: Option<String>,
    pub confidence: ArchitectureConfidence,
    pub sources: Vec<ArchitectureSource>,
    pub metadata: BTreeMap<String, String>,
}

impl ServiceIdentity {
    pub fn deterministic_id(project_id: &str, name: &str, kind: ServiceKind) -> String {
        stable_hash(&[
            "service",
            project_id,
            &normalize_service_name(name),
            kind.as_str(),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceKind {
    BackendApi,
    FrontendApp,
    WorkerService,
    LibraryPackage,
    DesktopApp,
    Infrastructure,
    Unknown,
}

impl ServiceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BackendApi => "BackendApi",
            Self::FrontendApp => "FrontendApp",
            Self::WorkerService => "WorkerService",
            Self::LibraryPackage => "LibraryPackage",
            Self::DesktopApp => "DesktopApp",
            Self::Infrastructure => "Infrastructure",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureNode {
    pub id: String,
    pub project_id: String,
    pub service_id: Option<String>,
    pub kind: ArchitectureNodeKind,
    pub name: String,
    pub label: String,
    pub path: Option<String>,
    pub symbol_id: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub confidence: ArchitectureConfidence,
    pub sources: Vec<ArchitectureSource>,
}

impl ArchitectureNode {
    pub fn deterministic_id(
        project_id: &str,
        service_id: Option<&str>,
        kind: ArchitectureNodeKind,
        name: &str,
        path: Option<&str>,
        symbol_id: Option<&str>,
    ) -> String {
        stable_hash(&[
            "node",
            project_id,
            service_id.unwrap_or(""),
            kind.as_str(),
            &normalize_service_name(name),
            path.unwrap_or(""),
            symbol_id.unwrap_or(""),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchitectureNodeKind {
    Project,
    Service,
    Route,
    Component,
    DataAccess,
    RealtimeEndpoint,
    MessagingTopic,
    MessagingQueue,
    MessagingExchange,
    MessagingRoutingKey,
    InfrastructureResource,
    Package,
    Contract,
    Database,
    External,
    Unknown,
}

impl ArchitectureNodeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Service => "Service",
            Self::Route => "Route",
            Self::Component => "Component",
            Self::DataAccess => "DataAccess",
            Self::RealtimeEndpoint => "RealtimeEndpoint",
            Self::MessagingTopic => "MessagingTopic",
            Self::MessagingQueue => "MessagingQueue",
            Self::MessagingExchange => "MessagingExchange",
            Self::MessagingRoutingKey => "MessagingRoutingKey",
            Self::InfrastructureResource => "InfrastructureResource",
            Self::Package => "Package",
            Self::Contract => "Contract",
            Self::Database => "Database",
            Self::External => "External",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureEdge {
    pub id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub kind: ArchitectureEdgeKind,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
    pub sources: Vec<ArchitectureSource>,
    pub metadata: BTreeMap<String, String>,
}

impl ArchitectureEdge {
    pub fn deterministic_id(
        from_node_id: &str,
        to_node_id: &str,
        kind: ArchitectureEdgeKind,
    ) -> String {
        stable_hash(&["edge", from_node_id, to_node_id, kind.as_str()])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchitectureEdgeKind {
    CallsHttpRoute,
    ExposesHttpRoute,
    UsesComponent,
    PublishesMessage,
    ConsumesMessage,
    ProducesRealtimeEvent,
    HandlesRealtimeEvent,
    UsesDataAccess,
    DeploysAs,
    SelectsWorkload,
    RoutesToService,
    ImportsPackage,
    ProvidesPackage,
    ImplementsContract,
    UsesInfrastructure,
    DependsOnService,
    Unknown,
}

impl ArchitectureEdgeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CallsHttpRoute => "CallsHttpRoute",
            Self::ExposesHttpRoute => "ExposesHttpRoute",
            Self::UsesComponent => "UsesComponent",
            Self::PublishesMessage => "PublishesMessage",
            Self::ConsumesMessage => "ConsumesMessage",
            Self::ProducesRealtimeEvent => "ProducesRealtimeEvent",
            Self::HandlesRealtimeEvent => "HandlesRealtimeEvent",
            Self::UsesDataAccess => "UsesDataAccess",
            Self::DeploysAs => "DeploysAs",
            Self::SelectsWorkload => "SelectsWorkload",
            Self::RoutesToService => "RoutesToService",
            Self::ImportsPackage => "ImportsPackage",
            Self::ProvidesPackage => "ProvidesPackage",
            Self::ImplementsContract => "ImplementsContract",
            Self::UsesInfrastructure => "UsesInfrastructure",
            Self::DependsOnService => "DependsOnService",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureMatchCandidate {
    pub id: String,
    pub left_project_id: String,
    pub right_project_id: Option<String>,
    pub left_node: ArchitectureNode,
    pub right_node: Option<ArchitectureNode>,
    pub relationship_kind: ArchitectureEdgeKind,
    pub match_key: String,
    pub normalized_key: String,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
    pub warnings: Vec<ArchitectureWarning>,
}

impl ArchitectureMatchCandidate {
    pub fn deterministic_id(
        left_project_id: &str,
        right_project_id: Option<&str>,
        relationship_kind: ArchitectureEdgeKind,
        normalized_key: &str,
    ) -> String {
        stable_hash(&[
            "match",
            left_project_id,
            right_project_id.unwrap_or(""),
            relationship_kind.as_str(),
            normalized_key,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ArchitectureConfidenceLevel {
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureConfidence {
    pub level: ArchitectureConfidenceLevel,
    pub score: u16,
    pub explanation: String,
    pub evidence: Vec<String>,
}

impl ArchitectureConfidence {
    pub const MAX_SCORE: u16 = 10_000;

    pub fn new(
        level: ArchitectureConfidenceLevel,
        score: u16,
        explanation: impl Into<String>,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            level,
            score: score.min(Self::MAX_SCORE),
            explanation: explanation.into(),
            evidence,
        }
    }

    pub fn high(explanation: impl Into<String>) -> Self {
        Self::new(
            ArchitectureConfidenceLevel::High,
            9_000,
            explanation,
            Vec::new(),
        )
    }

    pub fn medium(explanation: impl Into<String>) -> Self {
        Self::new(
            ArchitectureConfidenceLevel::Medium,
            6_000,
            explanation,
            Vec::new(),
        )
    }

    pub fn low(explanation: impl Into<String>) -> Self {
        Self::new(
            ArchitectureConfidenceLevel::Low,
            3_000,
            explanation,
            Vec::new(),
        )
    }

    pub fn unknown(explanation: impl Into<String>) -> Self {
        Self::new(
            ArchitectureConfidenceLevel::Unknown,
            0,
            explanation,
            Vec::new(),
        )
    }

    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureSource {
    pub project_id: String,
    pub file_path: String,
    pub symbol_id: Option<String>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub source_kind: ArchitectureSourceKind,
    pub extractor: Option<String>,
    pub metadata_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchitectureSourceKind {
    RouteMetadata,
    ComponentMetadata,
    DataAccessMetadata,
    RealtimeMetadata,
    MessagingMetadata,
    InfrastructureMetadata,
    WpfMetadata,
    GoSymbol,
    RegistryMetadata,
    UserGroupMetadata,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureEvidence {
    pub kind: ArchitectureEvidenceKind,
    pub description: String,
    pub value: Option<String>,
    pub source: Option<ArchitectureSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchitectureEvidenceKind {
    ExactLiteral,
    NormalizedKey,
    Metadata,
    Registry,
    NamingConvention,
    UserProvided,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureWarning {
    pub code: String,
    pub message: String,
    pub project_id: Option<String>,
}

impl ArchitectureWarning {
    pub fn missing_database(
        project_id: impl Into<String>,
        database_path: impl Into<String>,
    ) -> Self {
        let project_id = project_id.into();
        Self {
            code: "missing_database".to_string(),
            message: format!(
                "project database is missing or unavailable: {}",
                database_path.into()
            ),
            project_id: Some(project_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureCapabilityStatus {
    pub architecture_contracts_available: bool,
    pub group_federation_ready: bool,
    pub route_matching_ready: bool,
    pub messaging_matching_ready: bool,
    pub package_contract_infra_matching_ready: bool,
    pub group_impact_ready: bool,
    pub service_map_ready: bool,
    pub local_only: bool,
    pub global_db_merge_required: bool,
    pub global_database_required: bool,
    pub cloud_graph_database_required: bool,
    pub hosted_vector_database_required: bool,
    pub telemetry_enabled: bool,
}

impl Default for ArchitectureCapabilityStatus {
    fn default() -> Self {
        Self {
            architecture_contracts_available: true,
            group_federation_ready: true,
            route_matching_ready: false,
            messaging_matching_ready: false,
            package_contract_infra_matching_ready: false,
            group_impact_ready: false,
            service_map_ready: false,
            local_only: true,
            global_db_merge_required: false,
            global_database_required: false,
            cloud_graph_database_required: false,
            hosted_vector_database_required: false,
            telemetry_enabled: false,
        }
    }
}

pub fn normalize_http_method(method: &str) -> String {
    let method = method.trim().to_ascii_uppercase();
    if method.is_empty() {
        "GET".to_string()
    } else {
        method
    }
}

pub fn normalize_route_path(path: &str) -> String {
    let mut normalized = path
        .trim()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .replace('\\', "/");
    if normalized.is_empty() {
        return "/".to_string();
    }
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    if normalized.len() > 1 {
        normalized = normalized.trim_end_matches('/').to_string();
    }
    normalized.to_ascii_lowercase()
}

pub fn normalize_http_match_key(method: &str, path: &str) -> String {
    format!(
        "http:{}:{}",
        normalize_http_method(method),
        normalize_route_path(path)
    )
}

pub fn normalize_message_key(kind: &str, value: &str) -> String {
    format!(
        "messaging.{}:{}",
        normalize_key_part(kind),
        normalize_dotted_key(value)
    )
}

pub fn normalize_package_name(value: &str) -> String {
    let trimmed = value.trim();
    let without_prefix = trimmed
        .strip_prefix("package:")
        .or_else(|| trimmed.strip_prefix("Package:"))
        .or_else(|| trimmed.strip_prefix("PACKAGE:"))
        .unwrap_or(trimmed);
    normalize_dotted_key(without_prefix)
}

pub fn normalize_resource_key(kind: &str, value: &str) -> String {
    format!(
        "{}:{}",
        normalize_key_part(kind),
        normalize_dotted_key(value)
    )
}

pub fn normalize_service_name(value: &str) -> String {
    normalize_dotted_key(value).replace('.', "-")
}

fn normalize_key_part(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '.'
            }
        })
        .collect::<String>()
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn normalize_dotted_key(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace('\\', "/")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '.' | '/' | ':' | '@' | '_' | '-')
            {
                character
            } else {
                '.'
            }
        })
        .collect::<String>()
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> ArchitectureSource {
        ArchitectureSource {
            project_id: "api".to_string(),
            file_path: "src/routes.ts".to_string(),
            symbol_id: Some("symbol-route".to_string()),
            line_start: Some(10),
            line_end: Some(12),
            source_kind: ArchitectureSourceKind::RouteMetadata,
            extractor: Some("test".to_string()),
            metadata_key: Some("route.path".to_string()),
        }
    }

    #[test]
    fn project_and_group_identity_serialize_and_preserve_warnings() {
        let project = ProjectIdentity::new("api", "API", "services/api", ".b3/b3.db")
            .with_missing_database_warning();
        let group = ProjectGroupIdentity {
            group_id: "business-app".to_string(),
            name: "Business App".to_string(),
            project_ids: vec![project.project_id.clone()],
            created_from_registry: true,
            metadata: BTreeMap::from([("owner".to_string(), "local".to_string())]),
            warnings: Vec::new(),
        };

        let project_json = serde_json::to_string(&project).expect("project json");
        let group_json = serde_json::to_string(&group).expect("group json");

        assert!(project_json.contains("missing_database"));
        assert!(group_json.contains("business-app"));
    }

    #[test]
    fn service_identity_uses_deterministic_ids_and_sources() {
        let first = ServiceIdentity::deterministic_id("api", "Orders API", ServiceKind::BackendApi);
        let second =
            ServiceIdentity::deterministic_id("api", "orders-api", ServiceKind::BackendApi);
        let service = ServiceIdentity {
            service_id: first.clone(),
            project_id: "api".to_string(),
            name: "Orders API".to_string(),
            kind: ServiceKind::BackendApi,
            root_path: Some("services/api".to_string()),
            language_hints: vec!["typescript".to_string()],
            framework_hints: vec!["nestjs".to_string()],
            entrypoint_hints: vec!["src/main.ts".to_string()],
            infra_hints: Vec::new(),
            package_name: Some("@company/api".to_string()),
            docker_service_name: Some("api".to_string()),
            compose_service_name: None,
            kubernetes_service_name: None,
            confidence: ArchitectureConfidence::high("registry metadata"),
            sources: vec![source()],
            metadata: BTreeMap::new(),
        };

        assert_eq!(first, second);
        assert_eq!(service.sources[0].file_path, "src/routes.ts");
        assert_eq!(service.confidence.level, ArchitectureConfidenceLevel::High);
    }

    #[test]
    fn node_and_edge_ids_are_deterministic_and_serializable() {
        let node_id = ArchitectureNode::deterministic_id(
            "api",
            Some("service"),
            ArchitectureNodeKind::Route,
            "GET /orders",
            Some("src/routes.ts"),
            Some("route-symbol"),
        );
        let node = ArchitectureNode {
            id: node_id.clone(),
            project_id: "api".to_string(),
            service_id: Some("service".to_string()),
            kind: ArchitectureNodeKind::Route,
            name: "GET /orders".to_string(),
            label: "GET /orders".to_string(),
            path: Some("src/routes.ts".to_string()),
            symbol_id: Some("route-symbol".to_string()),
            metadata: BTreeMap::new(),
            confidence: ArchitectureConfidence::high("literal route"),
            sources: vec![source()],
        };
        let edge_id = ArchitectureEdge::deterministic_id(
            &node.id,
            "service-node",
            ArchitectureEdgeKind::ExposesHttpRoute,
        );
        let edge = ArchitectureEdge {
            id: edge_id.clone(),
            from_node_id: node.id.clone(),
            to_node_id: "service-node".to_string(),
            kind: ArchitectureEdgeKind::ExposesHttpRoute,
            confidence: ArchitectureConfidence::high("route metadata"),
            evidence: vec![ArchitectureEvidence {
                kind: ArchitectureEvidenceKind::ExactLiteral,
                description: "GET /orders".to_string(),
                value: Some(normalize_http_match_key("get", "/orders")),
                source: Some(source()),
            }],
            sources: node.sources.clone(),
            metadata: BTreeMap::new(),
        };

        assert_eq!(
            node_id,
            ArchitectureNode::deterministic_id(
                "api",
                Some("service"),
                ArchitectureNodeKind::Route,
                "GET /orders",
                Some("src/routes.ts"),
                Some("route-symbol"),
            )
        );
        assert_eq!(edge.id, edge_id);
        assert!(serde_json::to_string(&edge)
            .expect("edge json")
            .contains("ExposesHttpRoute"));
    }

    #[test]
    fn match_candidate_preserves_key_confidence_evidence_and_warnings() {
        let node = ArchitectureNode {
            id: "left".to_string(),
            project_id: "api".to_string(),
            service_id: None,
            kind: ArchitectureNodeKind::MessagingTopic,
            name: "order.created".to_string(),
            label: "order.created".to_string(),
            path: None,
            symbol_id: None,
            metadata: BTreeMap::new(),
            confidence: ArchitectureConfidence::medium("topic literal"),
            sources: vec![source()],
        };
        let normalized_key = normalize_message_key("topic", "Order.Created");
        let candidate = ArchitectureMatchCandidate {
            id: ArchitectureMatchCandidate::deterministic_id(
                "api",
                Some("worker"),
                ArchitectureEdgeKind::PublishesMessage,
                &normalized_key,
            ),
            left_project_id: "api".to_string(),
            right_project_id: Some("worker".to_string()),
            left_node: node,
            right_node: None,
            relationship_kind: ArchitectureEdgeKind::PublishesMessage,
            match_key: "messaging.topic:Order.Created".to_string(),
            normalized_key: normalized_key.clone(),
            confidence: ArchitectureConfidence::medium("normalized topic"),
            evidence: vec![ArchitectureEvidence {
                kind: ArchitectureEvidenceKind::NormalizedKey,
                description: "topic normalized".to_string(),
                value: Some(normalized_key.clone()),
                source: Some(source()),
            }],
            warnings: vec![ArchitectureWarning {
                code: "unmatched_peer".to_string(),
                message: "future matcher has not run".to_string(),
                project_id: Some("worker".to_string()),
            }],
        };

        assert_eq!(candidate.normalized_key, "messaging.topic:order.created");
        assert_eq!(
            candidate.confidence.level,
            ArchitectureConfidenceLevel::Medium
        );
        assert_eq!(candidate.warnings[0].code, "unmatched_peer");
    }

    #[test]
    fn normalization_helpers_are_deterministic() {
        assert_eq!(normalize_http_method(" post "), "POST");
        assert_eq!(normalize_route_path("api//Orders/"), "/api/orders");
        assert_eq!(
            normalize_http_match_key("get", "api/users/"),
            "http:GET:/api/users"
        );
        assert_eq!(
            normalize_message_key("routing key", " Order.Created "),
            "messaging.routing_key:order.created"
        );
        assert_eq!(
            normalize_package_name("Package:@Company/Shared-Contracts"),
            "@company/shared-contracts"
        );
        assert_eq!(
            normalize_resource_key("k8s:service", " API "),
            "k8s_service:api"
        );
        assert_eq!(normalize_service_name("Orders API"), "orders-api");
    }

    #[test]
    fn capability_status_is_contracts_only_and_local() {
        let status = ArchitectureCapabilityStatus::default();

        assert!(status.architecture_contracts_available);
        assert!(status.group_federation_ready);
        assert!(!status.route_matching_ready);
        assert!(!status.messaging_matching_ready);
        assert!(!status.package_contract_infra_matching_ready);
        assert!(!status.group_impact_ready);
        assert!(!status.service_map_ready);
        assert!(status.local_only);
        assert!(!status.global_db_merge_required);
        assert!(!status.global_database_required);
        assert!(!status.cloud_graph_database_required);
        assert!(!status.hosted_vector_database_required);
        assert!(!status.telemetry_enabled);
    }
}
