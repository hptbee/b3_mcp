use std::collections::{BTreeMap, BTreeSet};

use b3_core::{
    stable_hash, ArchitectureConfidence, ArchitectureConfidenceLevel, ArchitectureEdge,
    ArchitectureEdgeKind, ArchitectureEvidence, ArchitectureNode, ArchitectureNodeKind,
    ArchitectureSourceKind, ArchitectureWarning, ContractError, ContractResult,
};
use serde::{Deserialize, Serialize};

use super::{
    package_matching::DependencyMatchKindFilter, DependencyMatchOptions, GroupFederation,
    MessageMatchOptions, RouteMatchOptions, DEFAULT_BRANCH,
};

const DEFAULT_MAX_NODES: usize = 500;
const DEFAULT_MAX_EDGES: usize = 1_000;
const MAX_GRAPH_NODES: usize = 2_000;
const MAX_GRAPH_EDGES: usize = 5_000;
const DEFAULT_SERVICE_LIMIT: usize = 500;
const MAX_SERVICE_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureGraphRequest {
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub relationship_kinds: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub min_confidence: Option<GraphConfidenceFilter>,
    #[serde(default = "default_true")]
    pub include_evidence: bool,
    #[serde(default = "default_true")]
    pub include_warnings: bool,
    #[serde(default = "default_true")]
    pub include_unresolved: bool,
    #[serde(default)]
    pub max_nodes: Option<usize>,
    #[serde(default)]
    pub max_edges: Option<usize>,
    #[serde(default)]
    pub depth: Option<usize>,
    #[serde(default)]
    pub seed_project_id: Option<String>,
    #[serde(default)]
    pub seed_node_id: Option<String>,
    #[serde(default)]
    pub layout_hint: Option<String>,
}

impl Default for ArchitectureGraphRequest {
    fn default() -> Self {
        Self {
            branch: None,
            relationship_kinds: Vec::new(),
            project_ids: Vec::new(),
            min_confidence: None,
            include_evidence: true,
            include_warnings: true,
            include_unresolved: true,
            max_nodes: None,
            max_edges: None,
            depth: None,
            seed_project_id: None,
            seed_node_id: None,
            layout_hint: None,
        }
    }
}

impl ArchitectureGraphRequest {
    fn validate(&self) -> ContractResult<()> {
        validate_project_filters(&self.project_ids)?;
        if self.max_nodes == Some(0) {
            return Err(ContractError::new("max_nodes must be greater than zero"));
        }
        if self.max_edges == Some(0) {
            return Err(ContractError::new("max_edges must be greater than zero"));
        }
        for kind in &self.relationship_kinds {
            if parse_edge_kind(kind).is_none() {
                return Err(ContractError::new(format!(
                    "unsupported relationship kind: {kind}"
                )));
            }
        }
        validate_confidence_filter(self.min_confidence.as_ref())?;
        Ok(())
    }

    fn max_nodes(&self) -> usize {
        self.max_nodes
            .unwrap_or(DEFAULT_MAX_NODES)
            .min(MAX_GRAPH_NODES)
    }

    fn max_edges(&self) -> usize {
        self.max_edges
            .unwrap_or(DEFAULT_MAX_EDGES)
            .min(MAX_GRAPH_EDGES)
    }

    fn min_confidence_score(&self) -> Option<u16> {
        self.min_confidence
            .as_ref()
            .map(GraphConfidenceFilter::score)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMapRequest {
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub include_routes: bool,
    #[serde(default = "default_true")]
    pub include_messaging: bool,
    #[serde(default = "default_true")]
    pub include_dependencies: bool,
    #[serde(default = "default_true")]
    pub include_infrastructure: bool,
    #[serde(default)]
    pub min_confidence: Option<GraphConfidenceFilter>,
    #[serde(default = "default_true")]
    pub include_evidence: bool,
    #[serde(default = "default_true")]
    pub include_unresolved: bool,
    #[serde(default)]
    pub limit: Option<usize>,
}

impl Default for ServiceMapRequest {
    fn default() -> Self {
        Self {
            branch: None,
            project_ids: Vec::new(),
            include_routes: true,
            include_messaging: true,
            include_dependencies: true,
            include_infrastructure: true,
            min_confidence: None,
            include_evidence: true,
            include_unresolved: true,
            limit: None,
        }
    }
}

impl ServiceMapRequest {
    fn validate(&self) -> ContractResult<()> {
        validate_project_filters(&self.project_ids)?;
        if self.limit == Some(0) {
            return Err(ContractError::new("limit must be greater than zero"));
        }
        validate_confidence_filter(self.min_confidence.as_ref())?;
        Ok(())
    }

    fn limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_SERVICE_LIMIT)
            .min(MAX_SERVICE_LIMIT)
    }

    fn min_confidence_score(&self) -> Option<u16> {
        self.min_confidence
            .as_ref()
            .map(GraphConfidenceFilter::score)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GraphConfidenceFilter {
    Score(u16),
    Level(String),
}

impl GraphConfidenceFilter {
    fn score(&self) -> u16 {
        match self {
            Self::Score(score) => *score,
            Self::Level(level) => match level.trim().to_ascii_lowercase().as_str() {
                "high" => 8_000,
                "medium" => 5_000,
                "low" => 1,
                _ => 0,
            },
        }
    }
}

fn validate_confidence_filter(filter: Option<&GraphConfidenceFilter>) -> ContractResult<()> {
    let Some(GraphConfidenceFilter::Level(level)) = filter else {
        return Ok(());
    };
    match level.trim().to_ascii_lowercase().as_str() {
        "high" | "medium" | "low" => Ok(()),
        _ => Err(ContractError::new(
            "min_confidence must be high, medium, low, or a numeric score",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureGraphResult {
    pub group_id: String,
    pub group_name: String,
    pub branch: String,
    pub local_only: bool,
    pub graph_api_ready: bool,
    pub service_map_ready: bool,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub unresolved: Vec<UnresolvedRelationship>,
    pub summary: GraphSummary,
    pub warnings: Vec<ArchitectureWarning>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: ArchitectureNodeKind,
    pub label: String,
    pub name: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub file_path: Option<String>,
    pub symbol_id: Option<String>,
    pub source_kind: Option<ArchitectureSourceKind>,
    pub metadata: BTreeMap<String, String>,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub relationship_kind: ArchitectureEdgeKind,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
    pub source_phase: GraphEdgeSource,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeSource {
    RouteMatching,
    MessagingMatching,
    DependencyMatching,
    FederationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedRelationship {
    pub id: String,
    pub source_phase: GraphEdgeSource,
    pub project_id: String,
    pub relationship_kind: ArchitectureEdgeKind,
    pub name: String,
    pub warning: ArchitectureWarning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraphSummary {
    pub project_count: usize,
    pub service_count: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub route_edge_count: usize,
    pub messaging_edge_count: usize,
    pub dependency_edge_count: usize,
    pub infra_edge_count: usize,
    pub unresolved_count: usize,
    pub warning_count: usize,
    pub confidence_distribution: BTreeMap<String, usize>,
    pub top_connected_projects: Vec<ProjectConnectivity>,
    pub isolated_projects: Vec<String>,
    pub relationship_kind_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConnectivity {
    pub project_id: String,
    pub project_name: Option<String>,
    pub edge_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceMapResult {
    pub group_id: String,
    pub group_name: String,
    pub branch: String,
    pub local_only: bool,
    pub service_map_ready: bool,
    pub architecture_graph_api_ready: bool,
    pub services: Vec<ServiceSummary>,
    pub service_edges: Vec<ServiceMapEdge>,
    pub summary: GraphSummary,
    pub unresolved: Vec<UnresolvedRelationship>,
    pub warnings: Vec<ArchitectureWarning>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceSummary {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub service_id: String,
    pub service_name: String,
    pub service_kind: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub route_count: usize,
    pub messaging_count: usize,
    pub dependency_count: usize,
    pub infra_count: usize,
    pub inbound_edge_count: usize,
    pub outbound_edge_count: usize,
    pub confidence: ArchitectureConfidence,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceMapEdge {
    pub id: String,
    pub from_project_id: String,
    pub to_project_id: String,
    pub relationship_kind: ArchitectureEdgeKind,
    pub relationship_count: usize,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
    pub source_phases: Vec<GraphEdgeSource>,
    pub warnings: Vec<ArchitectureWarning>,
}

#[derive(Debug, Clone)]
struct GraphBuild {
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeMap<String, GraphEdge>,
    unresolved: Vec<UnresolvedRelationship>,
    warnings: Vec<ArchitectureWarning>,
}

impl GroupFederation {
    pub fn architecture_graph(
        &self,
        group_id: &str,
        request: ArchitectureGraphRequest,
    ) -> ContractResult<ArchitectureGraphResult> {
        request.validate()?;
        let context = self.resolve_context(group_id)?;
        let summary = self.summary(group_id)?;
        let branch = request
            .branch
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
        let mut build = build_architecture_graph(
            self,
            group_id,
            &branch,
            request.min_confidence_score(),
            graph_include_flags(&request),
            request.max_edges(),
        )?;
        build.warnings.extend(context.warnings.clone());
        insert_project_nodes(&mut build, &summary, &request.project_ids);
        apply_graph_filters(&mut build, &request);
        let mut nodes = build.nodes.into_values().collect::<Vec<_>>();
        let mut edges = build.edges.into_values().collect::<Vec<_>>();
        sort_nodes(&mut nodes);
        sort_edges(&mut edges);
        nodes.truncate(request.max_nodes());
        let node_ids = nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        edges.retain(|edge| {
            node_ids.contains(edge.from_node_id.as_str())
                && node_ids.contains(edge.to_node_id.as_str())
        });
        edges.truncate(request.max_edges());
        if !request.include_evidence {
            strip_evidence(&mut nodes, &mut edges);
        }
        let warnings = if request.include_warnings {
            build.warnings
        } else {
            Vec::new()
        };
        let unresolved = if request.include_unresolved {
            build.unresolved
        } else {
            Vec::new()
        };
        let graph_summary = summarize_graph(&nodes, &edges, &unresolved, warnings.len());
        Ok(ArchitectureGraphResult {
            group_id: summary.group_id,
            group_name: summary.group_name,
            branch,
            local_only: true,
            graph_api_ready: true,
            service_map_ready: true,
            nodes,
            edges,
            unresolved,
            summary: graph_summary,
            warnings,
            limitations: graph_limitations(),
        })
    }

    pub fn service_map(
        &self,
        group_id: &str,
        request: ServiceMapRequest,
    ) -> ContractResult<ServiceMapResult> {
        request.validate()?;
        let context = self.resolve_context(group_id)?;
        let summary = self.summary(group_id)?;
        let branch = request
            .branch
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
        let mut build = build_architecture_graph(
            self,
            group_id,
            &branch,
            request.min_confidence_score(),
            service_include_flags(&request),
            request.limit(),
        )?;
        build.warnings.extend(context.warnings.clone());
        insert_project_nodes(&mut build, &summary, &request.project_ids);
        apply_service_project_filter(&mut build, &request.project_ids);
        let mut nodes = build.nodes.into_values().collect::<Vec<_>>();
        let mut edges = build.edges.into_values().collect::<Vec<_>>();
        sort_nodes(&mut nodes);
        sort_edges(&mut edges);
        edges.truncate(request.limit());
        if !request.include_evidence {
            strip_evidence(&mut nodes, &mut edges);
        }
        let unresolved = if request.include_unresolved {
            build.unresolved
        } else {
            Vec::new()
        };
        let warnings = build.warnings;
        let services = summarize_services(&summary, &nodes, &edges, &request.project_ids);
        let service_edges = summarize_service_edges(&nodes, &edges);
        let graph_summary = summarize_graph(&nodes, &edges, &unresolved, warnings.len());
        Ok(ServiceMapResult {
            group_id: summary.group_id,
            group_name: summary.group_name,
            branch,
            local_only: true,
            service_map_ready: true,
            architecture_graph_api_ready: true,
            services,
            service_edges,
            summary: graph_summary,
            unresolved,
            warnings,
            limitations: graph_limitations(),
        })
    }
}

fn build_architecture_graph(
    federation: &GroupFederation,
    group_id: &str,
    branch: &str,
    min_confidence: Option<u16>,
    include: IncludeFlags,
    limit: usize,
) -> ContractResult<GraphBuild> {
    let mut build = GraphBuild {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
        unresolved: Vec::new(),
        warnings: Vec::new(),
    };
    if include.routes {
        let report = federation.route_matches(
            group_id,
            RouteMatchOptions {
                min_confidence,
                limit,
                branch: Some(branch.to_string()),
                ..RouteMatchOptions::default()
            },
        )?;
        build.warnings.extend(report.warnings);
        for matched in report.matches {
            insert_match(
                &mut build,
                matched.candidate.left_node,
                matched.candidate.right_node,
                matched.edge,
                GraphEdgeSource::RouteMatching,
                matched.candidate.evidence,
            );
        }
    }
    if include.messaging {
        let report = federation.message_matches(
            group_id,
            MessageMatchOptions {
                min_confidence,
                limit,
                branch: Some(branch.to_string()),
                ..MessageMatchOptions::default()
            },
        )?;
        build.warnings.extend(report.warnings);
        for matched in report.matches {
            insert_match(
                &mut build,
                matched.candidate.left_node,
                matched.candidate.right_node,
                matched.edge,
                GraphEdgeSource::MessagingMatching,
                matched.candidate.evidence,
            );
        }
    }
    if include.dependencies || include.infrastructure {
        let kind = match (include.dependencies, include.infrastructure) {
            (true, true) => DependencyMatchKindFilter::All,
            (true, false) => DependencyMatchKindFilter::All,
            (false, true) => DependencyMatchKindFilter::Infrastructure,
            (false, false) => DependencyMatchKindFilter::All,
        };
        let report = federation.dependency_matches(
            group_id,
            DependencyMatchOptions {
                kind,
                min_confidence,
                limit,
                branch: Some(branch.to_string()),
                ..DependencyMatchOptions::default()
            },
        )?;
        build.warnings.extend(report.warnings);
        for matched in report.matches {
            if !include.infrastructure && is_infra_edge(matched.edge.kind) {
                continue;
            }
            if !include.dependencies && !is_infra_edge(matched.edge.kind) {
                continue;
            }
            insert_match(
                &mut build,
                matched.candidate.left_node,
                matched.candidate.right_node,
                matched.edge,
                GraphEdgeSource::DependencyMatching,
                matched.candidate.evidence,
            );
        }
    }
    Ok(build)
}

fn insert_project_nodes(
    build: &mut GraphBuild,
    summary: &super::GroupArchitectureSummary,
    project_filter: &[String],
) {
    let project_filter = project_filter
        .iter()
        .map(|id| id.as_str())
        .collect::<BTreeSet<_>>();
    for project in &summary.projects {
        if !project_filter.is_empty() && !project_filter.contains(project.project_id.as_str()) {
            continue;
        }
        let mut metadata = BTreeMap::new();
        metadata.insert("root_path".to_string(), project.root_path.clone());
        metadata.insert("database_path".to_string(), project.database_path.clone());
        metadata.insert("status".to_string(), format!("{:?}", project.status));
        let id = project_node_id(&project.project_id);
        build.nodes.entry(id.clone()).or_insert(GraphNode {
            id,
            kind: ArchitectureNodeKind::Project,
            label: project.name.clone(),
            name: project.name.clone(),
            project_id: Some(project.project_id.clone()),
            project_name: Some(project.name.clone()),
            file_path: None,
            symbol_id: None,
            source_kind: Some(ArchitectureSourceKind::RegistryMetadata),
            metadata,
            confidence: ArchitectureConfidence::high("registry project identity"),
            evidence: Vec::new(),
            warnings: project.warnings.clone(),
        });
    }
}

fn insert_match(
    build: &mut GraphBuild,
    left: ArchitectureNode,
    right: Option<ArchitectureNode>,
    edge: ArchitectureEdge,
    source_phase: GraphEdgeSource,
    evidence: Vec<ArchitectureEvidence>,
) {
    let left_id = left.id.clone();
    build
        .nodes
        .entry(left_id.clone())
        .or_insert_with(|| GraphNode::from_architecture(&left, evidence.clone()));
    let Some(right) = right else {
        let warning = ArchitectureWarning {
            code: "unresolved_architecture_relationship".to_string(),
            message: format!(
                "no local target node for {} relationship {}",
                source_label(source_phase),
                edge.kind.as_str()
            ),
            project_id: Some(left.project_id.clone()),
        };
        build.unresolved.push(UnresolvedRelationship {
            id: stable_hash(&["unresolved", &left_id, edge.kind.as_str()]),
            source_phase,
            project_id: left.project_id,
            relationship_kind: edge.kind,
            name: left.name,
            warning,
        });
        return;
    };
    let right_id = right.id.clone();
    build
        .nodes
        .entry(right_id.clone())
        .or_insert_with(|| GraphNode::from_architecture(&right, evidence));
    let graph_edge = GraphEdge {
        id: edge.id,
        from_node_id: edge.from_node_id,
        to_node_id: edge.to_node_id,
        relationship_kind: edge.kind,
        confidence: edge.confidence,
        evidence: edge.evidence,
        source_phase,
        warnings: Vec::new(),
    };
    build.edges.insert(graph_edge.id.clone(), graph_edge);
}

impl GraphNode {
    fn from_architecture(node: &ArchitectureNode, evidence: Vec<ArchitectureEvidence>) -> Self {
        let source_kind = node.sources.first().map(|source| source.source_kind);
        Self {
            id: node.id.clone(),
            kind: node.kind,
            label: node.label.clone(),
            name: node.name.clone(),
            project_id: Some(node.project_id.clone()),
            project_name: node.metadata.get("project_name").cloned(),
            file_path: node.path.clone(),
            symbol_id: node.symbol_id.clone(),
            source_kind,
            metadata: node.metadata.clone(),
            confidence: node.confidence.clone(),
            evidence,
            warnings: Vec::new(),
        }
    }
}

fn apply_graph_filters(build: &mut GraphBuild, request: &ArchitectureGraphRequest) {
    apply_project_filter(build, &request.project_ids);
    if !request.relationship_kinds.is_empty() {
        let kinds = request
            .relationship_kinds
            .iter()
            .filter_map(|kind| parse_edge_kind(kind))
            .collect::<Vec<_>>();
        build
            .edges
            .retain(|_, edge| kinds.contains(&edge.relationship_kind));
    }
    if let Some(seed_project_id) = &request.seed_project_id {
        build.nodes.retain(|_, node| {
            node.project_id.as_deref() == Some(seed_project_id)
                || node.kind == ArchitectureNodeKind::Project
        });
    }
    if let Some(seed_node_id) = &request.seed_node_id {
        let related = related_node_ids(
            &build.edges,
            seed_node_id,
            request.depth.unwrap_or(1).min(5),
        );
        build.nodes.retain(|id, _| related.contains(id));
        build.edges.retain(|_, edge| {
            related.contains(&edge.from_node_id) && related.contains(&edge.to_node_id)
        });
    }
    retain_edges_with_nodes(build);
}

fn apply_service_project_filter(build: &mut GraphBuild, project_ids: &[String]) {
    apply_project_filter(build, project_ids);
    retain_edges_with_nodes(build);
}

fn apply_project_filter(build: &mut GraphBuild, project_ids: &[String]) {
    if project_ids.is_empty() {
        return;
    }
    let filters = project_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<BTreeSet<_>>();
    build.nodes.retain(|_, node| {
        node.project_id
            .as_deref()
            .is_some_and(|project_id| filters.contains(project_id))
    });
    build
        .unresolved
        .retain(|item| filters.contains(item.project_id.as_str()));
}

fn retain_edges_with_nodes(build: &mut GraphBuild) {
    let node_ids = build.nodes.keys().cloned().collect::<BTreeSet<_>>();
    build.edges.retain(|_, edge| {
        node_ids.contains(&edge.from_node_id) && node_ids.contains(&edge.to_node_id)
    });
}

fn related_node_ids(
    edges: &BTreeMap<String, GraphEdge>,
    seed_node_id: &str,
    depth: usize,
) -> BTreeSet<String> {
    let mut selected = BTreeSet::new();
    selected.insert(seed_node_id.to_string());
    for _ in 0..depth {
        let current = selected.clone();
        for edge in edges.values() {
            if current.contains(&edge.from_node_id) {
                selected.insert(edge.to_node_id.clone());
            }
            if current.contains(&edge.to_node_id) {
                selected.insert(edge.from_node_id.clone());
            }
        }
    }
    selected
}

fn summarize_graph(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    unresolved: &[UnresolvedRelationship],
    warning_count: usize,
) -> GraphSummary {
    let project_count = nodes
        .iter()
        .filter_map(|node| node.project_id.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    let service_count = nodes
        .iter()
        .filter(|node| {
            node.kind == ArchitectureNodeKind::Project || node.kind == ArchitectureNodeKind::Service
        })
        .count();
    let mut confidence_distribution = BTreeMap::new();
    for edge in edges {
        *confidence_distribution
            .entry(format!("{:?}", edge.confidence.level))
            .or_insert(0) += 1;
    }
    let mut relationship_kind_counts = BTreeMap::new();
    for edge in edges {
        *relationship_kind_counts
            .entry(edge.relationship_kind.as_str().to_string())
            .or_insert(0) += 1;
    }
    let mut project_edges = BTreeMap::<String, (Option<String>, usize)>::new();
    let node_projects = nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    for edge in edges {
        for node_id in [&edge.from_node_id, &edge.to_node_id] {
            if let Some(node) = node_projects.get(node_id.as_str()) {
                if let Some(project_id) = &node.project_id {
                    let entry = project_edges
                        .entry(project_id.clone())
                        .or_insert((node.project_name.clone(), 0));
                    entry.1 += 1;
                }
            }
        }
    }
    let all_projects = nodes
        .iter()
        .filter_map(|node| node.project_id.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut top_connected_projects = project_edges
        .iter()
        .map(
            |(project_id, (project_name, edge_count))| ProjectConnectivity {
                project_id: project_id.clone(),
                project_name: project_name.clone(),
                edge_count: *edge_count,
            },
        )
        .collect::<Vec<_>>();
    top_connected_projects.sort_by(|left, right| {
        right
            .edge_count
            .cmp(&left.edge_count)
            .then_with(|| left.project_id.cmp(&right.project_id))
    });
    top_connected_projects.truncate(10);
    let connected = project_edges.keys().cloned().collect::<BTreeSet<_>>();
    let isolated_projects = all_projects
        .difference(&connected)
        .cloned()
        .collect::<Vec<_>>();
    GraphSummary {
        project_count,
        service_count,
        node_count: nodes.len(),
        edge_count: edges.len(),
        route_edge_count: edges
            .iter()
            .filter(|edge| is_route_edge(edge.relationship_kind))
            .count(),
        messaging_edge_count: edges
            .iter()
            .filter(|edge| is_messaging_edge(edge.relationship_kind))
            .count(),
        dependency_edge_count: edges
            .iter()
            .filter(|edge| is_dependency_edge(edge.relationship_kind))
            .count(),
        infra_edge_count: edges
            .iter()
            .filter(|edge| is_infra_edge(edge.relationship_kind))
            .count(),
        unresolved_count: unresolved.len(),
        warning_count,
        confidence_distribution,
        top_connected_projects,
        isolated_projects,
        relationship_kind_counts,
    }
}

fn summarize_services(
    summary: &super::GroupArchitectureSummary,
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    project_filter: &[String],
) -> Vec<ServiceSummary> {
    let filter = project_filter
        .iter()
        .map(|id| id.as_str())
        .collect::<BTreeSet<_>>();
    let mut services = summary
        .projects
        .iter()
        .filter(|project| filter.is_empty() || filter.contains(project.project_id.as_str()))
        .map(|project| ServiceSummary {
            project_id: project.project_id.clone(),
            project_name: project.name.clone(),
            project_path: project.root_path.clone(),
            service_id: project_node_id(&project.project_id),
            service_name: project.name.clone(),
            service_kind: "project".to_string(),
            languages: Vec::new(),
            frameworks: Vec::new(),
            route_count: nodes
                .iter()
                .filter(|node| {
                    node.project_id.as_deref() == Some(&project.project_id)
                        && node.kind == ArchitectureNodeKind::Route
                })
                .count(),
            messaging_count: nodes
                .iter()
                .filter(|node| {
                    node.project_id.as_deref() == Some(&project.project_id)
                        && matches!(
                            node.kind,
                            ArchitectureNodeKind::MessagingTopic
                                | ArchitectureNodeKind::MessagingQueue
                                | ArchitectureNodeKind::MessagingExchange
                                | ArchitectureNodeKind::MessagingRoutingKey
                        )
                })
                .count(),
            dependency_count: nodes
                .iter()
                .filter(|node| {
                    node.project_id.as_deref() == Some(&project.project_id)
                        && matches!(
                            node.kind,
                            ArchitectureNodeKind::Package | ArchitectureNodeKind::Contract
                        )
                })
                .count(),
            infra_count: nodes
                .iter()
                .filter(|node| {
                    node.project_id.as_deref() == Some(&project.project_id)
                        && node.kind == ArchitectureNodeKind::InfrastructureResource
                })
                .count(),
            inbound_edge_count: 0,
            outbound_edge_count: 0,
            confidence: ArchitectureConfidence::medium("project-level service identity"),
            warnings: project.warnings.clone(),
        })
        .collect::<Vec<_>>();
    let node_by_id = nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    for service in &mut services {
        for edge in edges {
            let from_project = node_by_id
                .get(edge.from_node_id.as_str())
                .and_then(|node| node.project_id.as_deref());
            let to_project = node_by_id
                .get(edge.to_node_id.as_str())
                .and_then(|node| node.project_id.as_deref());
            if to_project == Some(service.project_id.as_str()) {
                service.inbound_edge_count += 1;
            }
            if from_project == Some(service.project_id.as_str()) {
                service.outbound_edge_count += 1;
            }
        }
    }
    services.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    services
}

fn summarize_service_edges(nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<ServiceMapEdge> {
    let node_by_id = nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<(String, String, String), Vec<&GraphEdge>>::new();
    for edge in edges {
        let Some(from_project) = node_by_id
            .get(edge.from_node_id.as_str())
            .and_then(|node| node.project_id.clone())
        else {
            continue;
        };
        let Some(to_project) = node_by_id
            .get(edge.to_node_id.as_str())
            .and_then(|node| node.project_id.clone())
        else {
            continue;
        };
        grouped
            .entry((
                from_project,
                to_project,
                edge.relationship_kind.as_str().to_string(),
            ))
            .or_default()
            .push(edge);
    }
    let mut output = grouped
        .into_iter()
        .filter_map(
            |((from_project_id, to_project_id, relationship_kind), edges)| {
                let relationship_kind = parse_edge_kind(&relationship_kind)?;
                let confidence = aggregate_confidence(edges.iter().map(|edge| &edge.confidence));
                let mut source_phases = edges
                    .iter()
                    .map(|edge| edge.source_phase)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                source_phases.sort();
                Some(ServiceMapEdge {
                    id: stable_hash(&[
                        "service-edge",
                        &from_project_id,
                        &to_project_id,
                        relationship_kind.as_str(),
                    ]),
                    from_project_id,
                    to_project_id,
                    relationship_kind,
                    relationship_count: edges.len(),
                    evidence: edges
                        .iter()
                        .flat_map(|edge| edge.evidence.iter().cloned())
                        .take(8)
                        .collect(),
                    confidence,
                    source_phases,
                    warnings: Vec::new(),
                })
            },
        )
        .collect::<Vec<_>>();
    output.sort_by(|left, right| {
        left.from_project_id
            .cmp(&right.from_project_id)
            .then_with(|| left.to_project_id.cmp(&right.to_project_id))
            .then_with(|| {
                left.relationship_kind
                    .as_str()
                    .cmp(right.relationship_kind.as_str())
            })
    });
    output
}

fn aggregate_confidence<'a>(
    values: impl Iterator<Item = &'a ArchitectureConfidence>,
) -> ArchitectureConfidence {
    let score = values.map(|confidence| confidence.score).min().unwrap_or(0);
    ArchitectureConfidence::new(
        confidence_level(score),
        score,
        "conservative service-map aggregation",
        Vec::new(),
    )
}

#[derive(Debug, Clone, Copy)]
struct IncludeFlags {
    routes: bool,
    messaging: bool,
    dependencies: bool,
    infrastructure: bool,
}

fn graph_include_flags(request: &ArchitectureGraphRequest) -> IncludeFlags {
    if request.relationship_kinds.is_empty() {
        return IncludeFlags {
            routes: true,
            messaging: true,
            dependencies: true,
            infrastructure: true,
        };
    }
    let kinds = request
        .relationship_kinds
        .iter()
        .filter_map(|kind| parse_edge_kind(kind))
        .collect::<Vec<_>>();
    IncludeFlags {
        routes: kinds.iter().any(|kind| is_route_edge(*kind)),
        messaging: kinds.iter().any(|kind| is_messaging_edge(*kind)),
        dependencies: kinds.iter().any(|kind| is_dependency_edge(*kind)),
        infrastructure: kinds.iter().any(|kind| is_infra_edge(*kind)),
    }
}

fn service_include_flags(request: &ServiceMapRequest) -> IncludeFlags {
    IncludeFlags {
        routes: request.include_routes,
        messaging: request.include_messaging,
        dependencies: request.include_dependencies,
        infrastructure: request.include_infrastructure,
    }
}

fn strip_evidence(nodes: &mut [GraphNode], edges: &mut [GraphEdge]) {
    for node in nodes {
        node.evidence.clear();
        node.confidence.evidence.clear();
    }
    for edge in edges {
        edge.evidence.clear();
        edge.confidence.evidence.clear();
    }
}

fn sort_nodes(nodes: &mut [GraphNode]) {
    nodes.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn sort_edges(edges: &mut [GraphEdge]) {
    edges.sort_by(|left, right| {
        left.from_node_id
            .cmp(&right.from_node_id)
            .then_with(|| left.to_node_id.cmp(&right.to_node_id))
            .then_with(|| {
                left.relationship_kind
                    .as_str()
                    .cmp(right.relationship_kind.as_str())
            })
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn project_node_id(project_id: &str) -> String {
    stable_hash(&["project-node", project_id])
}

fn is_route_edge(kind: ArchitectureEdgeKind) -> bool {
    matches!(
        kind,
        ArchitectureEdgeKind::CallsHttpRoute | ArchitectureEdgeKind::ExposesHttpRoute
    )
}

fn is_messaging_edge(kind: ArchitectureEdgeKind) -> bool {
    matches!(
        kind,
        ArchitectureEdgeKind::PublishesMessage | ArchitectureEdgeKind::ConsumesMessage
    )
}

fn is_dependency_edge(kind: ArchitectureEdgeKind) -> bool {
    matches!(
        kind,
        ArchitectureEdgeKind::ImportsPackage
            | ArchitectureEdgeKind::ProvidesPackage
            | ArchitectureEdgeKind::DependsOnPackage
            | ArchitectureEdgeKind::SharesContract
            | ArchitectureEdgeKind::UsesContract
            | ArchitectureEdgeKind::ImplementsContract
    )
}

fn is_infra_edge(kind: ArchitectureEdgeKind) -> bool {
    matches!(
        kind,
        ArchitectureEdgeKind::UsesInfrastructure
            | ArchitectureEdgeKind::DefinesInfrastructureResource
            | ArchitectureEdgeKind::DependsOnInfrastructure
            | ArchitectureEdgeKind::DeploysService
            | ArchitectureEdgeKind::SelectsService
            | ArchitectureEdgeKind::DeploysAs
            | ArchitectureEdgeKind::SelectsWorkload
            | ArchitectureEdgeKind::RoutesToService
    )
}

fn parse_edge_kind(value: &str) -> Option<ArchitectureEdgeKind> {
    let normalized = value
        .trim()
        .replace(['-', '_', ' '], "")
        .to_ascii_lowercase();
    [
        ArchitectureEdgeKind::CallsHttpRoute,
        ArchitectureEdgeKind::ExposesHttpRoute,
        ArchitectureEdgeKind::PublishesMessage,
        ArchitectureEdgeKind::ConsumesMessage,
        ArchitectureEdgeKind::ImportsPackage,
        ArchitectureEdgeKind::ProvidesPackage,
        ArchitectureEdgeKind::DependsOnPackage,
        ArchitectureEdgeKind::SharesContract,
        ArchitectureEdgeKind::UsesContract,
        ArchitectureEdgeKind::ImplementsContract,
        ArchitectureEdgeKind::UsesInfrastructure,
        ArchitectureEdgeKind::DefinesInfrastructureResource,
        ArchitectureEdgeKind::DependsOnInfrastructure,
        ArchitectureEdgeKind::DeploysService,
        ArchitectureEdgeKind::SelectsService,
        ArchitectureEdgeKind::DependsOnService,
        ArchitectureEdgeKind::Unknown,
    ]
    .into_iter()
    .find(|kind| kind.as_str().to_ascii_lowercase() == normalized)
}

fn validate_project_filters(project_ids: &[String]) -> ContractResult<()> {
    for project_id in project_ids {
        let trimmed = project_id.trim();
        if trimmed.is_empty()
            || trimmed.contains("..")
            || trimmed.contains('/')
            || trimmed.contains('\\')
        {
            return Err(ContractError::new(
                "project filters must be local registry project ids",
            ));
        }
    }
    Ok(())
}

fn graph_limitations() -> Vec<String> {
    vec![
        "static/read-only graph over local registry project DBs only".to_string(),
        "graph is built on demand and is not persisted or globally merged".to_string(),
        "no runtime HTTP, broker, package manager, Docker, Kubernetes, Terraform, cloud, or external API calls".to_string(),
        "architecture graph UI and Phase 11.7 benchmark expansion are not included".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

fn confidence_level(score: u16) -> ArchitectureConfidenceLevel {
    if score >= 8_000 {
        ArchitectureConfidenceLevel::High
    } else if score >= 5_000 {
        ArchitectureConfidenceLevel::Medium
    } else if score > 0 {
        ArchitectureConfidenceLevel::Low
    } else {
        ArchitectureConfidenceLevel::Unknown
    }
}

fn source_label(source: GraphEdgeSource) -> &'static str {
    match source {
        GraphEdgeSource::RouteMatching => "route matching",
        GraphEdgeSource::MessagingMatching => "messaging matching",
        GraphEdgeSource::DependencyMatching => "dependency matching",
        GraphEdgeSource::FederationSummary => "federation summary",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture::{LocalRegistryProject, DEFAULT_BRANCH};
    use b3_core::{
        BranchId, FileId, FileRecord, IndexStore, IndexedFileRecord, NodeKind, ProjectId, SymbolId,
        SymbolRecord,
    };
    use b3_storage::SqliteStorage;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write_registry(path: &Path, projects: &[(&str, &str, &Path)], group_projects: &[&str]) {
        let projects_json = projects
            .iter()
            .map(|(id, name, db)| {
                serde_json::to_string(&LocalRegistryProject {
                    id: id.to_string(),
                    name: name.to_string(),
                    path: db.parent().unwrap().display().to_string(),
                    database: db.display().to_string(),
                    tags: Vec::new(),
                    last_indexed_at: None,
                })
                .expect("project json")
            })
            .collect::<Vec<_>>()
            .join(",");
        let project_ids = group_projects
            .iter()
            .map(|id| format!(r#""{id}""#))
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            path,
            format!(
                r#"{{"version":1,"projects":[{projects_json}],"groups":[{{"id":"suite","name":"Suite","project_ids":[{project_ids}]}}]}}"#
            ),
        )
        .expect("registry");
    }

    fn seed_project(
        db: &Path,
        project_id: &str,
        file_path: &str,
        content: &str,
        symbols: Vec<SymbolRecord>,
    ) {
        let storage = SqliteStorage::open(db).expect("storage");
        let project = ProjectId::new(project_id);
        let branch = BranchId::new(DEFAULT_BRANCH);
        storage
            .ensure_project_branch(&project, &branch, &db.parent().unwrap().to_string_lossy())
            .expect("project");
        storage
            .upsert_indexed_file(
                &project,
                &branch,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project_id}-{file_path}")),
                        project_id: project.clone(),
                        path: file_path.to_string(),
                        content_hash: format!("hash-{project_id}-{file_path}"),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: content.len() as u64,
                    content: content.to_string(),
                    symbols,
                    edges: Vec::new(),
                },
            )
            .expect("indexed");
    }

    fn route_symbol(project_id: &str, method: &str, path: &str) -> SymbolRecord {
        let mut symbol = SymbolRecord::new(
            SymbolId::new(format!("{project_id}-route")),
            FileId::new(format!("{project_id}-src.ts")),
            format!("{method} {path}"),
            NodeKind::Route,
        );
        symbol.visibility = Some(format!(
            "route.framework=express;route.kind=api;route.method={method};route.path={path};route.file=src.ts;route.handler=handler;route.source=ExpressCall;route.line_start=1;route.line_end=1;route.confidence=9500"
        ));
        symbol
    }

    fn message_symbol(project_id: &str, direction: &str, topic: &str) -> SymbolRecord {
        let mut symbol = SymbolRecord::new(
            SymbolId::new(format!("{project_id}-message-{direction}")),
            FileId::new(format!("{project_id}-src.ts")),
            topic.to_string(),
            NodeKind::Endpoint,
        );
        let kind = if direction == "inbound" {
            "Consumer"
        } else {
            "Producer"
        };
        symbol.visibility = Some(format!(
            "messaging.technology=kafka;messaging.kind={kind};messaging.direction={direction};messaging.topic={topic};messaging.file=src.ts;messaging.source=TestMessaging;messaging.line_start=1;messaging.line_end=1;messaging.confidence=9000"
        ));
        symbol
    }

    #[test]
    fn graph_contains_project_route_message_and_package_edges() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        let worker_db = dir.path().join("worker").join(".b3").join("b3.db");
        let shared_db = dir.path().join("shared").join(".b3").join("b3.db");
        seed_project(
            &web_db,
            "web",
            "src.ts",
            r#"fetch("/api/orders");"#,
            Vec::new(),
        );
        seed_project(
            &api_db,
            "api",
            "src.ts",
            "app.get('/api/orders', handler); publish orders.created",
            vec![
                route_symbol("api", "GET", "/api/orders"),
                message_symbol("api", "outbound", "orders.created"),
            ],
        );
        seed_project(
            &worker_db,
            "worker",
            "src.ts",
            "consume orders.created",
            vec![message_symbol("worker", "inbound", "orders.created")],
        );
        seed_project(
            &shared_db,
            "shared",
            "package.json",
            r#"{"name":"shared-contracts"}"#,
            Vec::new(),
        );
        seed_project(
            &worker_db,
            "worker",
            "package.json",
            r#"{"dependencies":{"shared-contracts":"file:../shared"}}"#,
            Vec::new(),
        );
        write_registry(
            &registry,
            &[
                ("web", "Web", &web_db),
                ("api", "API", &api_db),
                ("worker", "Worker", &worker_db),
                ("shared", "Shared", &shared_db),
            ],
            &["web", "api", "worker", "shared"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let graph = federation
            .architecture_graph("suite", ArchitectureGraphRequest::default())
            .expect("graph");

        assert!(graph.local_only);
        assert!(graph.graph_api_ready);
        assert!(graph.service_map_ready);
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == ArchitectureNodeKind::Project
                && node.project_id.as_deref() == Some("api")));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.relationship_kind == ArchitectureEdgeKind::CallsHttpRoute));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.relationship_kind == ArchitectureEdgeKind::PublishesMessage));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.relationship_kind == ArchitectureEdgeKind::DependsOnPackage));
        assert!(graph.summary.project_count >= 4);
    }

    #[test]
    fn graph_filters_are_bounded_and_deterministic() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_project(
            &web_db,
            "web",
            "src.ts",
            r#"fetch("/api/orders");"#,
            Vec::new(),
        );
        seed_project(
            &api_db,
            "api",
            "src.ts",
            "app.get('/api/orders', handler);",
            vec![route_symbol("api", "GET", "/api/orders")],
        );
        write_registry(
            &registry,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let graph = federation
            .architecture_graph(
                "suite",
                ArchitectureGraphRequest {
                    relationship_kinds: vec!["CallsHttpRoute".to_string()],
                    max_nodes: Some(2),
                    max_edges: Some(1),
                    include_evidence: false,
                    ..ArchitectureGraphRequest::default()
                },
            )
            .expect("graph");
        assert!(graph.nodes.len() <= 2);
        assert!(graph.edges.len() <= 1);
        assert!(graph
            .edges
            .iter()
            .all(|edge| edge.relationship_kind == ArchitectureEdgeKind::CallsHttpRoute));
        assert!(graph.edges.iter().all(|edge| edge.evidence.is_empty()));
    }

    #[test]
    fn service_map_summarizes_project_edges_and_rejects_bad_filters() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_project(
            &web_db,
            "web",
            "src.ts",
            r#"fetch("/api/orders");"#,
            Vec::new(),
        );
        seed_project(
            &api_db,
            "api",
            "src.ts",
            "app.get('/api/orders', handler);",
            vec![route_symbol("api", "GET", "/api/orders")],
        );
        write_registry(
            &registry,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let map = federation
            .service_map("suite", ServiceMapRequest::default())
            .expect("service map");
        assert!(map.service_map_ready);
        assert!(map.architecture_graph_api_ready);
        assert_eq!(map.services.len(), 2);
        assert!(map.service_edges.iter().any(|edge| {
            edge.from_project_id == "web"
                && edge.to_project_id == "api"
                && edge.relationship_kind == ArchitectureEdgeKind::CallsHttpRoute
        }));
        let bad = ServiceMapRequest {
            project_ids: vec!["../bad".to_string()],
            ..ServiceMapRequest::default()
        };
        assert!(bad.validate().is_err());
        let bad_confidence = ArchitectureGraphRequest {
            min_confidence: Some(GraphConfidenceFilter::Level("certain".to_string())),
            ..ArchitectureGraphRequest::default()
        };
        assert!(bad_confidence.validate().is_err());
    }
}
