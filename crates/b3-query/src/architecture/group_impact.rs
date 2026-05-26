use std::collections::{BTreeMap, BTreeSet, VecDeque};

use b3_core::{
    stable_hash, ArchitectureConfidence, ArchitectureEdge, ArchitectureEdgeKind,
    ArchitectureEvidence, ArchitectureNode, ArchitectureNodeKind, ArchitectureSourceKind,
    ArchitectureWarning, ContractError, ContractResult,
};
use serde::{Deserialize, Serialize};

use super::{
    package_matching::DependencyMatchKindFilter, DependencyMatchOptions, GroupFederation,
    MessageMatchOptions, RouteMatchOptions, DEFAULT_BRANCH, DEFAULT_LIMIT,
};

const DEFAULT_MAX_DEPTH: usize = 2;
const MAX_DEPTH: usize = 5;
const MAX_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupImpactSeedType {
    File,
    Symbol,
    Route,
    Message,
    Package,
    Contract,
    Infrastructure,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupImpactDirection {
    Upstream,
    Downstream,
    Both,
}

impl Default for GroupImpactDirection {
    fn default() -> Self {
        Self::Downstream
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPackProfile {
    Minimal,
    Balanced,
    Deep,
}

impl Default for ContextPackProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

impl ContextPackProfile {
    fn char_budget(self) -> usize {
        match self {
            Self::Minimal => 4_000,
            Self::Balanced => 10_000,
            Self::Deep => 20_000,
        }
    }

    fn max_snippet_chars(self) -> usize {
        match self {
            Self::Minimal => 360,
            Self::Balanced => 720,
            Self::Deep => 1_200,
        }
    }

    fn max_paths(self) -> usize {
        match self {
            Self::Minimal => 5,
            Self::Balanced => 12,
            Self::Deep => 25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfidenceFilter {
    Score(u16),
    Level(String),
}

impl ConfidenceFilter {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupImpactRequest {
    pub seed_type: GroupImpactSeedType,
    #[serde(default)]
    pub seed_project_id: Option<String>,
    #[serde(default)]
    pub seed_path: Option<String>,
    #[serde(default)]
    pub seed_symbol: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub route_path: Option<String>,
    #[serde(default)]
    pub message_name: Option<String>,
    #[serde(default)]
    pub package_name: Option<String>,
    #[serde(default)]
    pub contract_name: Option<String>,
    #[serde(default)]
    pub infra_name: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub direction: GroupImpactDirection,
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub context_profile: ContextPackProfile,
    #[serde(default)]
    pub include_context_pack: bool,
    #[serde(default)]
    pub min_confidence: Option<ConfidenceFilter>,
}

impl GroupImpactRequest {
    fn validate(&self) -> ContractResult<()> {
        if let Some(path) = &self.seed_path {
            validate_relative_path(path)?;
        }
        if self.limit.unwrap_or(DEFAULT_LIMIT) == 0 {
            return Err(ContractError::new("limit must be greater than zero"));
        }
        if self.max_depth.unwrap_or(DEFAULT_MAX_DEPTH) == 0 {
            return Err(ContractError::new("max_depth must be greater than zero"));
        }
        let has_seed = match self.seed_type {
            GroupImpactSeedType::File => self.seed_path.as_deref().is_some_and(|v| !v.is_empty()),
            GroupImpactSeedType::Symbol => {
                self.seed_symbol.as_deref().is_some_and(|v| !v.is_empty())
            }
            GroupImpactSeedType::Route => self.route_path.as_deref().is_some_and(|v| !v.is_empty()),
            GroupImpactSeedType::Message => {
                self.message_name.as_deref().is_some_and(|v| !v.is_empty())
            }
            GroupImpactSeedType::Package => {
                self.package_name.as_deref().is_some_and(|v| !v.is_empty())
            }
            GroupImpactSeedType::Contract => {
                self.contract_name.as_deref().is_some_and(|v| !v.is_empty())
            }
            GroupImpactSeedType::Infrastructure => {
                self.infra_name.as_deref().is_some_and(|v| !v.is_empty())
            }
            GroupImpactSeedType::Query => self.query.as_deref().is_some_and(|v| !v.is_empty()),
        };
        if !has_seed {
            return Err(ContractError::new("impact seed is required"));
        }
        Ok(())
    }

    fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }

    fn max_depth(&self) -> usize {
        self.max_depth
            .unwrap_or(DEFAULT_MAX_DEPTH)
            .clamp(1, MAX_DEPTH)
    }

    fn min_confidence_score(&self) -> Option<u16> {
        self.min_confidence.as_ref().map(ConfidenceFilter::score)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupImpactResult {
    pub group_id: String,
    pub group_name: String,
    pub seed: ImpactSeed,
    pub direction: GroupImpactDirection,
    pub max_depth: usize,
    pub local_only: bool,
    pub impacted_project_count: usize,
    pub impacted_file_count: usize,
    pub impacted_symbol_count: usize,
    pub impacted_route_count: usize,
    pub impacted_message_count: usize,
    pub impacted_package_count: usize,
    pub impacted_contract_count: usize,
    pub impacted_infrastructure_count: usize,
    pub nodes: Vec<ImpactNode>,
    pub edges: Vec<ImpactEdge>,
    pub impact_paths: Vec<ImpactPath>,
    pub summary_by_project: Vec<ProjectImpactSummary>,
    pub warnings: Vec<ArchitectureWarning>,
    pub limitations: Vec<String>,
    pub context_pack: Option<GroupContextPack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactSeed {
    pub seed_type: GroupImpactSeedType,
    pub project_id: Option<String>,
    pub path: Option<String>,
    pub symbol: Option<String>,
    pub name: String,
    pub candidate_count: usize,
    pub selected_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactNode {
    pub id: String,
    pub node_kind: ArchitectureNodeKind,
    pub project_id: String,
    pub project_name: Option<String>,
    pub file_path: Option<String>,
    pub symbol_id: Option<String>,
    pub name: String,
    pub source_kind: Option<ArchitectureSourceKind>,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
}

impl ImpactNode {
    fn from_architecture(node: &ArchitectureNode, evidence: Vec<ArchitectureEvidence>) -> Self {
        let source_kind = node.sources.first().map(|source| source.source_kind);
        Self {
            id: node.id.clone(),
            node_kind: node.kind,
            project_id: node.project_id.clone(),
            project_name: node.metadata.get("project_name").cloned(),
            file_path: node.path.clone(),
            symbol_id: node.symbol_id.clone(),
            name: node.name.clone(),
            source_kind,
            confidence: node.confidence.clone(),
            evidence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactEdge {
    pub id: String,
    pub relationship_kind: ArchitectureEdgeKind,
    pub from_node_id: String,
    pub to_node_id: String,
    pub confidence: ArchitectureConfidence,
    pub evidence: Vec<ArchitectureEvidence>,
    pub source_phase: ImpactEdgeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactEdgeSource {
    RouteMatching,
    MessagingMatching,
    DependencyMatching,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactPath {
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub depth: usize,
    pub confidence: ArchitectureConfidence,
    pub reason: String,
    pub evidence_summary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectImpactSummary {
    pub project_id: String,
    pub project_name: Option<String>,
    pub node_count: usize,
    pub file_count: usize,
    pub symbol_count: usize,
    pub relationship_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupContextPack {
    pub profile: ContextPackProfile,
    pub char_budget: usize,
    pub returned_chars: usize,
    pub estimated_tokens: usize,
    pub sections: Vec<ContextPackSection>,
    pub snippets: Vec<ContextSnippet>,
    pub skipped_items: Vec<String>,
    pub truncation_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPackSection {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnippet {
    pub project_id: String,
    pub file_path: String,
    pub symbol_id: Option<String>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub snippet: String,
    pub why: String,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone)]
struct ImpactGraph {
    nodes: BTreeMap<String, ImpactNode>,
    edges: BTreeMap<String, ImpactEdge>,
    adjacency: BTreeMap<String, Vec<(String, String)>>,
    reverse_adjacency: BTreeMap<String, Vec<(String, String)>>,
    warnings: Vec<ArchitectureWarning>,
}

impl GroupFederation {
    pub fn group_impact(
        &self,
        group_id: &str,
        request: GroupImpactRequest,
    ) -> ContractResult<GroupImpactResult> {
        request.validate()?;
        let context = self.resolve_context(group_id)?;
        let branch = request
            .branch
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
        let graph = build_graph(self, group_id, &branch, &request)?;
        let mut warnings = context.warnings.clone();
        warnings.extend(graph.warnings.clone());
        let seeds = resolve_seed_nodes(&graph, &request);
        if seeds.is_empty() {
            return Err(ContractError::new(
                "impact seed did not match any local entity",
            ));
        }

        let paths = traverse_impact(
            &graph,
            &seeds,
            request.direction,
            request.max_depth(),
            request.limit(),
        );
        let mut node_ids = BTreeSet::new();
        for seed in &seeds {
            node_ids.insert(seed.clone());
        }
        let mut edge_ids = BTreeSet::new();
        for path in &paths {
            node_ids.extend(path.node_ids.iter().cloned());
            edge_ids.extend(path.edge_ids.iter().cloned());
        }
        let mut nodes = node_ids
            .iter()
            .filter_map(|id| graph.nodes.get(id).cloned())
            .collect::<Vec<_>>();
        let mut edges = edge_ids
            .iter()
            .filter_map(|id| graph.edges.get(id).cloned())
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            left.project_id
                .cmp(&right.project_id)
                .then_with(|| left.file_path.cmp(&right.file_path))
                .then_with(|| left.name.cmp(&right.name))
        });
        edges.sort_by(|left, right| {
            left.from_node_id
                .cmp(&right.from_node_id)
                .then_with(|| left.to_node_id.cmp(&right.to_node_id))
                .then_with(|| {
                    left.relationship_kind
                        .as_str()
                        .cmp(right.relationship_kind.as_str())
                })
        });
        let summaries = summarize_projects(&nodes, &edges);
        let context_pack = if request.include_context_pack {
            Some(build_context_pack(
                self, group_id, &branch, &request, &nodes, &edges, &paths,
            )?)
        } else {
            None
        };
        let seed_name = seed_name(&request);
        let seed = ImpactSeed {
            seed_type: request.seed_type,
            project_id: request.seed_project_id.clone(),
            path: request.seed_path.clone(),
            symbol: request.seed_symbol.clone(),
            name: seed_name,
            candidate_count: seeds.len(),
            selected_node_ids: seeds,
        };

        Ok(GroupImpactResult {
            group_id: context.group_id,
            group_name: context.group_name,
            seed,
            direction: request.direction,
            max_depth: request.max_depth(),
            local_only: true,
            impacted_project_count: summaries.len(),
            impacted_file_count: nodes
                .iter()
                .filter_map(|node| node.file_path.as_ref().map(|path| (&node.project_id, path)))
                .collect::<BTreeSet<_>>()
                .len(),
            impacted_symbol_count: nodes.iter().filter(|node| node.symbol_id.is_some()).count(),
            impacted_route_count: nodes.iter().filter(|node| node.node_kind == ArchitectureNodeKind::Route).count(),
            impacted_message_count: nodes
                .iter()
                .filter(|node| matches!(node.node_kind, ArchitectureNodeKind::MessagingTopic | ArchitectureNodeKind::MessagingQueue | ArchitectureNodeKind::MessagingExchange | ArchitectureNodeKind::MessagingRoutingKey))
                .count(),
            impacted_package_count: nodes.iter().filter(|node| node.node_kind == ArchitectureNodeKind::Package).count(),
            impacted_contract_count: nodes.iter().filter(|node| node.node_kind == ArchitectureNodeKind::Contract).count(),
            impacted_infrastructure_count: nodes.iter().filter(|node| node.node_kind == ArchitectureNodeKind::InfrastructureResource).count(),
            nodes,
            edges,
            impact_paths: paths,
            summary_by_project: summaries,
            warnings,
            limitations: vec![
                "static/read-only impact over local match candidates only".to_string(),
                "no runtime HTTP, broker, package manager, Docker, Kubernetes, Terraform, or cloud calls".to_string(),
                "no schema compatibility validation or service-map API in Phase 11.5".to_string(),
            ],
            context_pack,
        })
    }
}

fn build_graph(
    federation: &GroupFederation,
    group_id: &str,
    branch: &str,
    request: &GroupImpactRequest,
) -> ContractResult<ImpactGraph> {
    let min_confidence = request.min_confidence_score();
    let route_report = federation.route_matches(
        group_id,
        RouteMatchOptions {
            min_confidence,
            limit: request.limit(),
            branch: Some(branch.to_string()),
            ..RouteMatchOptions::default()
        },
    )?;
    let message_report = federation.message_matches(
        group_id,
        MessageMatchOptions {
            min_confidence,
            limit: request.limit(),
            branch: Some(branch.to_string()),
            ..MessageMatchOptions::default()
        },
    )?;
    let dependency_report = federation.dependency_matches(
        group_id,
        DependencyMatchOptions {
            kind: DependencyMatchKindFilter::All,
            min_confidence,
            limit: request.limit(),
            branch: Some(branch.to_string()),
            ..DependencyMatchOptions::default()
        },
    )?;

    let mut graph = ImpactGraph {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        reverse_adjacency: BTreeMap::new(),
        warnings: Vec::new(),
    };
    graph.warnings.extend(route_report.warnings);
    graph.warnings.extend(message_report.warnings);
    graph.warnings.extend(dependency_report.warnings);
    for matched in route_report.matches {
        insert_match(
            &mut graph,
            matched.candidate.left_node,
            matched.candidate.right_node,
            matched.edge,
            ImpactEdgeSource::RouteMatching,
            matched.candidate.evidence,
        );
    }
    for matched in message_report.matches {
        insert_match(
            &mut graph,
            matched.candidate.left_node,
            matched.candidate.right_node,
            matched.edge,
            ImpactEdgeSource::MessagingMatching,
            matched.candidate.evidence,
        );
    }
    for matched in dependency_report.matches {
        insert_match(
            &mut graph,
            matched.candidate.left_node,
            matched.candidate.right_node,
            matched.edge,
            ImpactEdgeSource::DependencyMatching,
            matched.candidate.evidence,
        );
    }
    Ok(graph)
}

fn insert_match(
    graph: &mut ImpactGraph,
    left: ArchitectureNode,
    right: Option<ArchitectureNode>,
    edge: ArchitectureEdge,
    source_phase: ImpactEdgeSource,
    evidence: Vec<ArchitectureEvidence>,
) {
    let left_id = left.id.clone();
    graph
        .nodes
        .entry(left_id.clone())
        .or_insert_with(|| ImpactNode::from_architecture(&left, evidence.clone()));
    let Some(right) = right else {
        return;
    };
    let right_id = right.id.clone();
    graph
        .nodes
        .entry(right_id.clone())
        .or_insert_with(|| ImpactNode::from_architecture(&right, evidence.clone()));
    let impact_edge = ImpactEdge {
        id: edge.id,
        relationship_kind: edge.kind,
        from_node_id: edge.from_node_id,
        to_node_id: edge.to_node_id,
        confidence: edge.confidence,
        evidence: edge.evidence,
        source_phase,
    };
    graph
        .adjacency
        .entry(impact_edge.from_node_id.clone())
        .or_default()
        .push((impact_edge.to_node_id.clone(), impact_edge.id.clone()));
    graph
        .reverse_adjacency
        .entry(impact_edge.to_node_id.clone())
        .or_default()
        .push((impact_edge.from_node_id.clone(), impact_edge.id.clone()));
    graph.edges.insert(impact_edge.id.clone(), impact_edge);
}

fn resolve_seed_nodes(graph: &ImpactGraph, request: &GroupImpactRequest) -> Vec<String> {
    let mut seeds = graph
        .nodes
        .values()
        .filter(|node| {
            if let Some(project_id) = &request.seed_project_id {
                if &node.project_id != project_id {
                    return false;
                }
            }
            match request.seed_type {
                GroupImpactSeedType::File => request
                    .seed_path
                    .as_deref()
                    .is_some_and(|path| node.file_path.as_deref() == Some(path)),
                GroupImpactSeedType::Symbol => {
                    request.seed_symbol.as_deref().is_some_and(|symbol| {
                        node.symbol_id.as_deref() == Some(symbol)
                            || node.name.eq_ignore_ascii_case(symbol)
                    })
                }
                GroupImpactSeedType::Route => {
                    node.node_kind == ArchitectureNodeKind::Route
                        && request.route_path.as_deref().is_some_and(|path| {
                            node.name.contains(path)
                                || node
                                    .file_path
                                    .as_deref()
                                    .is_some_and(|file| file.contains(path))
                        })
                        && request.method.as_deref().is_none_or(|method| {
                            node.name
                                .to_ascii_uppercase()
                                .contains(&method.to_ascii_uppercase())
                        })
                }
                GroupImpactSeedType::Message => {
                    request.message_name.as_deref().is_some_and(|name| {
                        node.name.eq_ignore_ascii_case(name)
                            || node
                                .name
                                .to_ascii_lowercase()
                                .contains(&name.to_ascii_lowercase())
                    })
                }
                GroupImpactSeedType::Package => {
                    node.node_kind == ArchitectureNodeKind::Package
                        && request.package_name.as_deref().is_some_and(|name| {
                            node.name.eq_ignore_ascii_case(name)
                                || node
                                    .name
                                    .to_ascii_lowercase()
                                    .contains(&name.to_ascii_lowercase())
                        })
                }
                GroupImpactSeedType::Contract => {
                    node.node_kind == ArchitectureNodeKind::Contract
                        && request.contract_name.as_deref().is_some_and(|name| {
                            node.name.eq_ignore_ascii_case(name)
                                || node
                                    .name
                                    .to_ascii_lowercase()
                                    .contains(&name.to_ascii_lowercase())
                        })
                }
                GroupImpactSeedType::Infrastructure => {
                    node.node_kind == ArchitectureNodeKind::InfrastructureResource
                        && request.infra_name.as_deref().is_some_and(|name| {
                            node.name.eq_ignore_ascii_case(name)
                                || node
                                    .name
                                    .to_ascii_lowercase()
                                    .contains(&name.to_ascii_lowercase())
                        })
                }
                GroupImpactSeedType::Query => request.query.as_deref().is_some_and(|query| {
                    let query = query.to_ascii_lowercase();
                    node.name.to_ascii_lowercase().contains(&query)
                        || node
                            .file_path
                            .as_deref()
                            .is_some_and(|path| path.to_ascii_lowercase().contains(&query))
                }),
            }
        })
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    seeds.sort();
    seeds
}

fn traverse_impact(
    graph: &ImpactGraph,
    seeds: &[String],
    direction: GroupImpactDirection,
    max_depth: usize,
    limit: usize,
) -> Vec<ImpactPath> {
    let mut paths = Vec::new();
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    for seed in seeds {
        queue.push_back((
            seed.clone(),
            vec![seed.clone()],
            Vec::<String>::new(),
            0usize,
            10_000u16,
        ));
        seen.insert((seed.clone(), 0usize));
    }
    while let Some((node_id, node_path, edge_path, depth, score)) = queue.pop_front() {
        if depth >= max_depth || paths.len() >= limit {
            continue;
        }
        for (next_node, edge_id) in neighbors(graph, &node_id, direction) {
            if node_path.contains(&next_node) {
                continue;
            }
            let Some(edge) = graph.edges.get(&edge_id) else {
                continue;
            };
            let next_score = score
                .min(edge.confidence.score)
                .saturating_sub((depth as u16) * 250);
            let mut next_node_path = node_path.clone();
            next_node_path.push(next_node.clone());
            let mut next_edge_path = edge_path.clone();
            next_edge_path.push(edge_id.clone());
            let path = ImpactPath {
                node_ids: next_node_path.clone(),
                edge_ids: next_edge_path.clone(),
                depth: depth + 1,
                confidence: ArchitectureConfidence::new(
                    confidence_level(next_score),
                    next_score,
                    "bounded group impact path confidence",
                    vec![edge.relationship_kind.as_str().to_string()],
                ),
                reason: format!(
                    "{} via {}",
                    impact_direction_label(direction),
                    edge.relationship_kind.as_str()
                ),
                evidence_summary: edge
                    .evidence
                    .iter()
                    .filter_map(|evidence| {
                        evidence
                            .value
                            .clone()
                            .or(Some(evidence.description.clone()))
                    })
                    .take(4)
                    .collect(),
            };
            paths.push(path);
            if seen.insert((next_node.clone(), depth + 1)) {
                queue.push_back((
                    next_node,
                    next_node_path,
                    next_edge_path,
                    depth + 1,
                    next_score,
                ));
            }
            if paths.len() >= limit {
                break;
            }
        }
    }
    paths.sort_by(|left, right| {
        right
            .confidence
            .score
            .cmp(&left.confidence.score)
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| left.node_ids.cmp(&right.node_ids))
    });
    paths
}

fn neighbors(
    graph: &ImpactGraph,
    node_id: &str,
    direction: GroupImpactDirection,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    if matches!(
        direction,
        GroupImpactDirection::Upstream | GroupImpactDirection::Both
    ) {
        result.extend(graph.adjacency.get(node_id).cloned().unwrap_or_default());
    }
    if matches!(
        direction,
        GroupImpactDirection::Downstream | GroupImpactDirection::Both
    ) {
        result.extend(
            graph
                .reverse_adjacency
                .get(node_id)
                .cloned()
                .unwrap_or_default(),
        );
        result.extend(
            graph
                .adjacency
                .get(node_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|(_, edge_id)| {
                    graph.edges.get(edge_id).is_some_and(|edge| {
                        matches!(
                            edge.relationship_kind,
                            ArchitectureEdgeKind::PublishesMessage
                                | ArchitectureEdgeKind::SharesContract
                                | ArchitectureEdgeKind::UsesContract
                                | ArchitectureEdgeKind::DependsOnInfrastructure
                                | ArchitectureEdgeKind::DeploysService
                                | ArchitectureEdgeKind::SelectsService
                        )
                    })
                }),
        );
    }
    result.sort();
    result.dedup();
    result
}

fn summarize_projects(nodes: &[ImpactNode], edges: &[ImpactEdge]) -> Vec<ProjectImpactSummary> {
    let mut map = BTreeMap::<String, ProjectImpactSummary>::new();
    for node in nodes {
        let entry = map
            .entry(node.project_id.clone())
            .or_insert(ProjectImpactSummary {
                project_id: node.project_id.clone(),
                project_name: node.project_name.clone(),
                node_count: 0,
                file_count: 0,
                symbol_count: 0,
                relationship_count: 0,
            });
        entry.node_count += 1;
        if node.file_path.is_some() {
            entry.file_count += 1;
        }
        if node.symbol_id.is_some() {
            entry.symbol_count += 1;
        }
    }
    for edge in edges {
        if let Some(node) = nodes.iter().find(|node| node.id == edge.from_node_id) {
            if let Some(entry) = map.get_mut(&node.project_id) {
                entry.relationship_count += 1;
            }
        }
    }
    map.into_values().collect()
}

fn build_context_pack(
    federation: &GroupFederation,
    group_id: &str,
    branch: &str,
    request: &GroupImpactRequest,
    nodes: &[ImpactNode],
    edges: &[ImpactEdge],
    paths: &[ImpactPath],
) -> ContractResult<GroupContextPack> {
    let profile = request.context_profile;
    let budget = profile.char_budget();
    let mut sections = Vec::new();
    sections.push(ContextPackSection {
        title: "seed".to_string(),
        content: seed_name(request),
    });
    sections.push(ContextPackSection {
        title: "high-level impact summary".to_string(),
        content: format!(
            "{} impacted projects, {} nodes, {} relationships, max_depth={}",
            nodes
                .iter()
                .map(|node| node.project_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            nodes.len(),
            edges.len(),
            request.max_depth()
        ),
    });
    sections.push(ContextPackSection {
        title: "cross-project relationships".to_string(),
        content: paths
            .iter()
            .take(profile.max_paths())
            .map(|path| {
                format!(
                    "depth {} score {} {}",
                    path.depth, path.confidence.score, path.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    });
    let mut snippets = Vec::new();
    let mut skipped = Vec::new();
    let files = collect_files_by_project(federation, group_id, branch)?;
    let mut seen = BTreeSet::new();
    for node in nodes {
        let Some(path) = &node.file_path else {
            continue;
        };
        let key = format!("{}\0{}", node.project_id, path);
        if !seen.insert(key) {
            continue;
        }
        let Some(content) = files
            .get(&node.project_id)
            .and_then(|project_files| project_files.get(path))
        else {
            skipped.push(format!(
                "{}:{} missing indexed file content",
                node.project_id, path
            ));
            continue;
        };
        let snippet = bounded_snippet(
            content,
            node.confidence.evidence.first(),
            profile.max_snippet_chars(),
        );
        snippets.push(ContextSnippet {
            project_id: node.project_id.clone(),
            file_path: path.clone(),
            symbol_id: node.symbol_id.clone(),
            line_start: None,
            line_end: None,
            estimated_tokens: estimate_tokens(&snippet),
            snippet,
            why: format!("impacted {} {}", node.node_kind.as_str(), node.name),
        });
    }
    let mut returned = sections
        .iter()
        .map(|section| section.title.len() + section.content.len())
        .sum::<usize>();
    let mut kept_snippets = Vec::new();
    for snippet in snippets {
        let size = snippet.snippet.len() + snippet.why.len() + snippet.file_path.len();
        if returned + size > budget {
            skipped.push(format!(
                "{}:{} budget limit",
                snippet.project_id, snippet.file_path
            ));
            continue;
        }
        returned += size;
        kept_snippets.push(snippet);
    }
    let truncation_reason =
        (!skipped.is_empty()).then(|| "context pack budget skipped some items".to_string());
    Ok(GroupContextPack {
        profile,
        char_budget: budget,
        returned_chars: returned,
        estimated_tokens: estimate_tokens(&"x".repeat(returned)),
        sections,
        snippets: kept_snippets,
        skipped_items: skipped,
        truncation_reason,
    })
}

fn collect_files_by_project(
    federation: &GroupFederation,
    group_id: &str,
    branch: &str,
) -> ContractResult<BTreeMap<String, BTreeMap<String, String>>> {
    let context = federation.resolve_context(group_id)?;
    let mut output = BTreeMap::new();
    for handle in context
        .projects
        .iter()
        .filter(|project| project.status == super::FederatedProjectStatus::Ready)
    {
        let storage = super::open_existing_read_only(handle)?;
        let files = storage.file_contents(&handle.project_id, branch, 2_000)?;
        output.insert(
            handle.project_id.clone(),
            files
                .into_iter()
                .map(|file| (file.path, file.content))
                .collect(),
        );
    }
    Ok(output)
}

fn bounded_snippet(content: &str, needle: Option<&String>, max_chars: usize) -> String {
    let start = needle
        .and_then(|needle| {
            content
                .to_ascii_lowercase()
                .find(&needle.to_ascii_lowercase())
        })
        .unwrap_or(0);
    let start = start.saturating_sub(max_chars / 4);
    content
        .chars()
        .skip(start)
        .take(max_chars)
        .collect::<String>()
}

fn seed_name(request: &GroupImpactRequest) -> String {
    request
        .route_path
        .clone()
        .or(request.message_name.clone())
        .or(request.package_name.clone())
        .or(request.contract_name.clone())
        .or(request.infra_name.clone())
        .or(request.seed_path.clone())
        .or(request.seed_symbol.clone())
        .or(request.query.clone())
        .unwrap_or_else(|| "impact seed".to_string())
}

fn validate_relative_path(path: &str) -> ContractResult<()> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.contains(':')
    {
        return Err(ContractError::new(
            "seed_path must be a relative local path",
        ));
    }
    Ok(())
}

fn confidence_level(score: u16) -> b3_core::ArchitectureConfidenceLevel {
    if score >= 8_000 {
        b3_core::ArchitectureConfidenceLevel::High
    } else if score >= 5_000 {
        b3_core::ArchitectureConfidenceLevel::Medium
    } else if score > 0 {
        b3_core::ArchitectureConfidenceLevel::Low
    } else {
        b3_core::ArchitectureConfidenceLevel::Unknown
    }
}

fn impact_direction_label(direction: GroupImpactDirection) -> &'static str {
    match direction {
        GroupImpactDirection::Upstream => "upstream impact",
        GroupImpactDirection::Downstream => "downstream impact",
        GroupImpactDirection::Both => "bidirectional impact",
    }
}

fn estimate_tokens(value: &str) -> usize {
    value.len().div_ceil(4)
}

#[allow(dead_code)]
fn deterministic_context_id(parts: &[&str]) -> String {
    stable_hash(parts)
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
        symbol.start_line = 1;
        symbol.end_line = 1;
        symbol.visibility = Some(format!(
            "route.framework=express;route.kind=api;route.method={method};route.path={path};route.file=src.ts;route.handler=handler;route.source=ExpressCall;route.line_start=1;route.line_end=1;route.confidence=9500"
        ));
        symbol
    }

    fn message_symbol(project_id: &str, direction: &str, topic: &str) -> SymbolRecord {
        let mut symbol = SymbolRecord::new(
            SymbolId::new(format!("{project_id}-message")),
            FileId::new(format!("{project_id}-src.ts")),
            topic.to_string(),
            NodeKind::Endpoint,
        );
        symbol.start_line = 1;
        symbol.end_line = 1;
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
    fn route_seed_impacts_frontend_and_context_pack_is_bounded() {
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
        let report = federation
            .group_impact(
                "suite",
                GroupImpactRequest {
                    seed_type: GroupImpactSeedType::Route,
                    route_path: Some("/api/orders".to_string()),
                    method: Some("GET".to_string()),
                    direction: GroupImpactDirection::Downstream,
                    include_context_pack: true,
                    context_profile: ContextPackProfile::Minimal,
                    ..request_defaults()
                },
            )
            .expect("impact");

        assert!(report.local_only);
        assert!(report.impacted_project_count >= 2);
        assert!(report
            .edges
            .iter()
            .any(|edge| edge.relationship_kind == ArchitectureEdgeKind::CallsHttpRoute));
        let pack = report.context_pack.expect("context pack");
        assert!(pack.returned_chars <= pack.char_budget);
        assert!(pack.sections.iter().any(|section| section.title == "seed"));
    }

    #[test]
    fn message_and_package_seeds_traverse_cross_project_matches() {
        let dir = TempDir::new().expect("tempdir");
        let registry = dir.path().join("registry.json");
        let producer_db = dir.path().join("producer").join(".b3").join("b3.db");
        let consumer_db = dir.path().join("consumer").join(".b3").join("b3.db");
        let shared_db = dir.path().join("shared").join(".b3").join("b3.db");
        seed_project(
            &producer_db,
            "producer",
            "src.ts",
            "publish orders.created",
            vec![message_symbol("producer", "outbound", "orders.created")],
        );
        seed_project(
            &consumer_db,
            "consumer",
            "package.json",
            r#"{"name":"consumer","dependencies":{"shared-contracts":"file:../shared"}}"#,
            Vec::new(),
        );
        seed_project(
            &consumer_db,
            "consumer",
            "src.ts",
            "consume orders.created",
            vec![message_symbol("consumer", "inbound", "orders.created")],
        );
        seed_project(
            &shared_db,
            "shared",
            "package.json",
            r#"{"name":"shared-contracts"}"#,
            Vec::new(),
        );
        write_registry(
            &registry,
            &[
                ("producer", "Producer", &producer_db),
                ("consumer", "Consumer", &consumer_db),
                ("shared", "Shared", &shared_db),
            ],
            &["producer", "consumer", "shared"],
        );
        let federation = GroupFederation::from_registry_path(&registry).expect("federation");
        let message = federation
            .group_impact(
                "suite",
                GroupImpactRequest {
                    seed_type: GroupImpactSeedType::Message,
                    message_name: Some("orders.created".to_string()),
                    direction: GroupImpactDirection::Downstream,
                    ..request_defaults()
                },
            )
            .expect("message impact");
        assert!(message
            .edges
            .iter()
            .any(|edge| edge.relationship_kind == ArchitectureEdgeKind::PublishesMessage));

        let package = federation
            .group_impact(
                "suite",
                GroupImpactRequest {
                    seed_type: GroupImpactSeedType::Package,
                    package_name: Some("shared-contracts".to_string()),
                    direction: GroupImpactDirection::Downstream,
                    ..request_defaults()
                },
            )
            .expect("package impact");
        assert!(package
            .edges
            .iter()
            .any(|edge| edge.relationship_kind == ArchitectureEdgeKind::DependsOnPackage));
    }

    #[test]
    fn validates_path_traversal_and_missing_seed() {
        let mut request = request_defaults();
        request.seed_type = GroupImpactSeedType::File;
        request.seed_path = Some("../secret".to_string());
        assert!(request.validate().is_err());
        request.seed_path = None;
        assert!(request.validate().is_err());
    }

    fn request_defaults() -> GroupImpactRequest {
        GroupImpactRequest {
            seed_type: GroupImpactSeedType::Query,
            seed_project_id: None,
            seed_path: None,
            seed_symbol: None,
            method: None,
            route_path: None,
            message_name: None,
            package_name: None,
            contract_name: None,
            infra_name: None,
            query: Some("seed".to_string()),
            branch: None,
            direction: GroupImpactDirection::Downstream,
            max_depth: Some(2),
            limit: Some(100),
            context_profile: ContextPackProfile::Balanced,
            include_context_pack: false,
            min_confidence: None,
        }
    }
}
