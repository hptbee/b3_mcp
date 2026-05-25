//! Hybrid query and graph retrieval boundary.
//!
//! Query code orchestrates exact lookup, FTS/BM25, graph expansion, ranking,
//! token savings estimates, and context packing through core traits. It does
//! not own storage clients, embedding workers, MCP request handling, or UI.

pub mod hybrid;

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
};

use b3_core::{
    CentralityMetric, CentralityRepository, CentralitySnapshot, ContextItem, ContextItemDto,
    ContextPack, ContextPackResponse, ContractError, ContractResult, EdgeKind, EdgeProvenance,
    FindCalleesResponse, FindCallersResponse, FindSymbolResponse, GraphDirection, GraphNeighbor,
    ImpactAnalysisResponse, ImpactRiskLevel, ImpactRiskSignalDto, NodeKind, QueryEngine,
    QueryIntent, QueryRepository, QueryRequest, QueryResult, QuerySavingsEstimate,
    QuerySavingsEstimateDto, QueryScope, QuerySymbol, QueryTraceDto, RankingWeights,
    RelatedSymbolsResponse, RelatedTestDto, RetrievalConfig, SearchCodeResponse, SymbolDto,
    SymbolId, TokenSavingsRecord, TokenSavingsRepository, TraversalStepDto, VectorStore,
};

use crate::hybrid::{HybridSearchEngine, HybridSearchRequest, HybridSearchResponse};

pub use b3_core::{
    FtsSearchHit as LexicalSearchHit, GraphDirection as TraversalDirection,
    GraphNeighbor as TraversalNeighbor, QueryScope as QueryEngineScope,
};

const DEFAULT_FTS_LIMIT: usize = 20;
const DEFAULT_GRAPH_LIMIT: usize = 64;
const DEFAULT_CYCLE_NODE_LIMIT: usize = 512;
const CENTRALITY_ALGORITHM_VERSION: &str = "pagerank-v1";

#[derive(Debug, Clone, PartialEq)]
pub struct QueryEngineConfig {
    pub retrieval: RetrievalConfig,
    pub ranking: RankingWeights,
    pub min_edge_confidence: u16,
    pub centrality: CentralityConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CentralityConfig {
    pub max_nodes: usize,
    pub iterations: usize,
    pub damping_factor: f64,
    pub convergence_threshold: f64,
    pub edge_filter: Vec<EdgeKind>,
}

impl Default for CentralityConfig {
    fn default() -> Self {
        Self {
            max_nodes: 512,
            iterations: 20,
            damping_factor: 0.85,
            convergence_threshold: 0.0001,
            edge_filter: vec![
                EdgeKind::Calls,
                EdgeKind::References,
                EdgeKind::Imports,
                EdgeKind::DependsOn,
                EdgeKind::Implements,
                EdgeKind::Inherits,
            ],
        }
    }
}

impl Default for QueryEngineConfig {
    fn default() -> Self {
        Self {
            retrieval: RetrievalConfig::default(),
            ranking: RankingWeights {
                semantic_similarity: 0,
                ..RankingWeights::default()
            },
            min_edge_confidence: 0,
            centrality: CentralityConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedSymbol {
    pub symbol: QuerySymbol,
    pub score: i64,
    pub why: String,
    pub graph_distance: usize,
    pub ranking_decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraversalStep {
    pub symbol: QuerySymbol,
    pub via: GraphNeighbor,
    pub distance: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DependencyPath {
    pub found: bool,
    pub nodes: Vec<QuerySymbol>,
    pub edges: Vec<GraphNeighbor>,
    pub path_length: usize,
    pub confidence_summary: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleGroup {
    pub node_ids: Vec<SymbolId>,
    pub edge_types: Vec<EdgeKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleDetectionResult {
    pub cycles: Vec<CycleGroup>,
    pub scanned_nodes: usize,
    pub summary_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct ImpactRiskAssessment {
    score: u16,
    level: ImpactRiskLevel,
    reasons: Vec<String>,
    signals: Vec<ImpactRiskSignalDto>,
    related_tests: Vec<RelatedTestDto>,
    impacted_files: Vec<String>,
    dependency_paths: Vec<Vec<String>>,
    cycles_involved: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RankingProfile {
    exact_match: i64,
    bm25: i64,
    graph_distance: i64,
    edge_confidence: i64,
    symbol_kind: i64,
    provenance: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct PackCandidate {
    ranked: RankedSymbol,
    file_path: String,
    snippet: String,
    estimated_tokens: usize,
    value_per_token: f64,
    penalties: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryTraceBuilder {
    trace: QueryTraceDto,
}

impl QueryTraceBuilder {
    fn new(scope: &QueryScope, query_input: impl Into<String>, query_intent: &str) -> Self {
        let query_input = query_input.into();
        Self {
            trace: QueryTraceDto {
                trace_id: stable_trace_id(scope, query_intent, &query_input),
                query_input,
                query_intent: query_intent.to_string(),
                project_id: scope.project_id.as_str().to_string(),
                branch_id: scope.branch_id.as_str().to_string(),
                exact_symbol_hits: Vec::new(),
                fts_hits: Vec::new(),
                graph_traversal_steps: Vec::new(),
                ranking_decisions: Vec::new(),
                context_items_selected: Vec::new(),
                context_items_skipped: Vec::new(),
                truncation_reason: None,
                token_budget_used: 0,
                token_budget: 0,
                token_savings_estimate: None,
                warnings: Vec::new(),
            },
        }
    }

    fn finish(self) -> QueryTraceDto {
        self.trace
    }
}

pub struct LocalQueryEngine<R> {
    repository: R,
    config: QueryEngineConfig,
}

impl<R> LocalQueryEngine<R>
where
    R: QueryRepository + TokenSavingsRepository + CentralityRepository,
{
    pub fn new(repository: R, config: QueryEngineConfig) -> Self {
        Self { repository, config }
    }

    pub fn classify_intent(&self, query: &str) -> QueryIntent {
        classify_query_intent(query)
    }

    pub fn find_symbol(&self, scope: &QueryScope, name: &str) -> ContractResult<Vec<RankedSymbol>> {
        let intent = QueryIntent::SymbolLookup;
        let mut symbols = self
            .repository
            .find_symbols(scope, name)?
            .into_iter()
            .map(|symbol| rank_symbol(symbol, name, intent, self.config.ranking, None, 0))
            .collect::<Vec<_>>();
        self.apply_centrality_boost(scope, &mut symbols)?;

        symbols.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        Ok(symbols)
    }

    pub fn find_symbol_response(
        &self,
        scope: &QueryScope,
        name: &str,
        include_trace: bool,
    ) -> ContractResult<FindSymbolResponse> {
        let mut trace = QueryTraceBuilder::new(scope, name, "find_symbol");
        let symbols = self.find_symbol(scope, name)?;
        for ranked in &symbols {
            trace
                .trace
                .exact_symbol_hits
                .push(ranked.symbol.id.as_str().to_string());
            trace.trace.ranking_decisions.push(format!(
                "{} score={} reason={} decision={}",
                ranked.symbol.name, ranked.score, ranked.why, ranked.ranking_decision
            ));
        }
        let trace = trace.finish();
        Ok(FindSymbolResponse {
            symbols: symbols.iter().map(symbol_dto).collect(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    pub fn search_code(
        &self,
        scope: &QueryScope,
        query: &str,
        limit: usize,
    ) -> ContractResult<Vec<RankedSymbol>> {
        let mut ranked = Vec::new();
        let mut seen = HashSet::new();
        let intent = classify_query_intent(query);

        for hit in self.repository.fts_search(scope, query, limit)? {
            if let Some(symbol_id) = hit.symbol_id {
                if seen.insert(symbol_id.as_str().to_string()) {
                    if let Some(symbol) = self.repository.get_symbol(scope, &symbol_id)? {
                        ranked.push(rank_symbol(
                            symbol,
                            query,
                            intent,
                            self.config.ranking,
                            Some(hit.score),
                            0,
                        ));
                    }
                }
            }
        }
        self.apply_centrality_boost(scope, &mut ranked)?;

        ranked.sort_by(|left, right| right.score.cmp(&left.score));
        Ok(ranked)
    }

    pub fn search_code_response(
        &self,
        scope: &QueryScope,
        query: &str,
        limit: usize,
        include_trace: bool,
    ) -> ContractResult<SearchCodeResponse> {
        let mut trace = QueryTraceBuilder::new(scope, query, "search_code");
        let symbols = self.search_code(scope, query, limit)?;
        for ranked in &symbols {
            trace
                .trace
                .fts_hits
                .push(ranked.symbol.id.as_str().to_string());
            trace.trace.ranking_decisions.push(format!(
                "{} score={} reason={} decision={}",
                ranked.symbol.name, ranked.score, ranked.why, ranked.ranking_decision
            ));
        }
        let trace = trace.finish();
        Ok(SearchCodeResponse {
            symbols: symbols.iter().map(symbol_dto).collect(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    pub fn find_callers(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        max_depth: usize,
    ) -> ContractResult<Vec<TraversalStep>> {
        self.traverse(
            scope,
            symbol_id,
            GraphDirection::Inbound,
            &[EdgeKind::Calls],
            max_depth,
        )
    }

    pub fn find_callers_response(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        max_depth: usize,
        include_trace: bool,
    ) -> ContractResult<FindCallersResponse> {
        let mut trace = QueryTraceBuilder::new(scope, symbol_id.as_str(), "find_callers");
        let callers = self.find_callers(scope, symbol_id, max_depth)?;
        record_steps(&mut trace, &callers);
        let trace = trace.finish();
        Ok(FindCallersResponse {
            callers: callers
                .iter()
                .map(|step| traversal_step_dto(step, "inbound"))
                .collect(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    pub fn find_callees(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        max_depth: usize,
    ) -> ContractResult<Vec<TraversalStep>> {
        self.traverse(
            scope,
            symbol_id,
            GraphDirection::Outbound,
            &[EdgeKind::Calls],
            max_depth,
        )
    }

    pub fn find_callees_response(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        max_depth: usize,
        include_trace: bool,
    ) -> ContractResult<FindCalleesResponse> {
        let mut trace = QueryTraceBuilder::new(scope, symbol_id.as_str(), "find_callees");
        let callees = self.find_callees(scope, symbol_id, max_depth)?;
        record_steps(&mut trace, &callees);
        let trace = trace.finish();
        Ok(FindCalleesResponse {
            callees: callees
                .iter()
                .map(|step| traversal_step_dto(step, "outbound"))
                .collect(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    pub fn related_symbols(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        max_depth: usize,
    ) -> ContractResult<Vec<TraversalStep>> {
        self.traverse(
            scope,
            symbol_id,
            GraphDirection::Both,
            &[EdgeKind::Contains, EdgeKind::Imports, EdgeKind::Calls],
            max_depth,
        )
    }

    pub fn related_symbols_response(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        max_depth: usize,
        include_trace: bool,
    ) -> ContractResult<RelatedSymbolsResponse> {
        let mut trace = QueryTraceBuilder::new(scope, symbol_id.as_str(), "related_symbols");
        let related = self.related_symbols(scope, symbol_id, max_depth)?;
        record_steps(&mut trace, &related);
        let trace = trace.finish();
        Ok(RelatedSymbolsResponse {
            related: related
                .iter()
                .map(|step| traversal_step_dto(step, "both"))
                .collect(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    pub fn impact_analysis(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Vec<TraversalStep>> {
        let mut impact = self.find_callers(
            scope,
            symbol_id,
            self.config.retrieval.max_graph_depth as usize,
        )?;
        impact.extend(self.related_symbols(scope, symbol_id, 1)?);
        dedupe_steps(&mut impact);
        impact.truncate(DEFAULT_GRAPH_LIMIT);
        Ok(impact)
    }

    pub fn impact_analysis_response(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        include_trace: bool,
    ) -> ContractResult<ImpactAnalysisResponse> {
        let mut trace = QueryTraceBuilder::new(scope, symbol_id.as_str(), "impact_analysis");
        let impacted = self.impact_analysis(scope, symbol_id)?;
        record_steps(&mut trace, &impacted);
        trace
            .trace
            .graph_traversal_steps
            .push(format!("seed symbol/file={}", symbol_id.as_str()));
        trace.trace.graph_traversal_steps.push(format!(
            "traversal depth={}",
            self.config.retrieval.max_graph_depth
        ));
        let assessment = self.assess_impact(scope, symbol_id, &impacted, &mut trace)?;
        let impacted_symbols = impacted
            .iter()
            .map(|step| {
                symbol_dto(&RankedSymbol {
                    symbol: step.symbol.clone(),
                    score: traversal_score(step),
                    why: "impacted by graph traversal".to_string(),
                    graph_distance: step.distance,
                    ranking_decision: format!(
                        "impact distance={} confidence={}",
                        step.distance,
                        step.via.confidence.basis_points()
                    ),
                })
            })
            .collect();
        let trace = trace.finish();
        Ok(ImpactAnalysisResponse {
            impacted: impacted
                .iter()
                .map(|step| traversal_step_dto(step, "both"))
                .collect(),
            risk_score: assessment.score,
            risk_level: assessment.level,
            risk_reasons: assessment.reasons,
            risk_signals: assessment.signals,
            impacted_symbols,
            impacted_files: assessment.impacted_files,
            related_tests: assessment.related_tests.clone(),
            missing_tests: assessment.related_tests.is_empty(),
            dependency_paths: assessment.dependency_paths,
            cycles_involved: assessment.cycles_involved,
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    fn assess_impact(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        impacted: &[TraversalStep],
        trace: &mut QueryTraceBuilder,
    ) -> ContractResult<ImpactRiskAssessment> {
        let seed = self.repository.get_symbol(scope, symbol_id)?;
        let fan_in = self
            .repository
            .graph_neighbors(
                scope,
                symbol_id,
                GraphDirection::Inbound,
                &[EdgeKind::Calls],
                self.config.min_edge_confidence,
            )?
            .len();
        let fan_out = self
            .repository
            .graph_neighbors(
                scope,
                symbol_id,
                GraphDirection::Outbound,
                &[EdgeKind::Calls],
                self.config.min_edge_confidence,
            )?
            .len();
        let public_api = seed
            .as_ref()
            .map(public_api_exposure)
            .unwrap_or_else(|| (false, "seed symbol missing".to_string()));
        let centrality = self.repository.get_centrality_metric(scope, symbol_id)?;
        let related_tests = self.related_tests(scope, symbol_id, seed.as_ref(), impacted, trace)?;
        let cycles = self.detect_cycles(scope, &[EdgeKind::Calls], DEFAULT_CYCLE_NODE_LIMIT, 0)?;
        let impacted_ids = impacted
            .iter()
            .map(|step| step.symbol.id.as_str().to_string())
            .chain(std::iter::once(symbol_id.as_str().to_string()))
            .collect::<HashSet<_>>();
        let cycles_involved = cycles
            .cycles
            .into_iter()
            .filter(|cycle| {
                cycle
                    .node_ids
                    .iter()
                    .any(|node| impacted_ids.contains(node.as_str()))
            })
            .map(|cycle| {
                cycle
                    .node_ids
                    .into_iter()
                    .map(|node| node.as_str().to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let impacted_files = self.impacted_file_paths(scope, seed.as_ref(), impacted)?;
        let max_distance = impacted.iter().map(|step| step.distance).max().unwrap_or(0);
        let min_confidence = impacted
            .iter()
            .map(|step| step.via.confidence.basis_points())
            .min()
            .unwrap_or(10_000);
        let dependency_paths = impacted
            .iter()
            .take(8)
            .filter_map(|step| {
                let from = step.via.from_symbol.as_ref()?;
                let to = step.via.to_symbol.as_ref()?;
                Some(vec![from.as_str().to_string(), to.as_str().to_string()])
            })
            .collect::<Vec<_>>();

        let mut signals = Vec::new();
        signals.push(risk_signal(
            "fan_in",
            fan_in as i64,
            10,
            (fan_in as i64 * 10).min(35),
            "more inbound callers increase refactor blast radius",
        ));
        signals.push(risk_signal(
            "fan_out",
            fan_out as i64,
            5,
            (fan_out as i64 * 5).min(20),
            "more outbound calls increase dependency sensitivity",
        ));
        signals.push(risk_signal(
            "graph_distance",
            max_distance as i64,
            4,
            if impacted.is_empty() {
                0
            } else if max_distance <= 1 {
                12
            } else {
                6
            },
            "near graph neighbors are more likely to be affected",
        ));
        signals.push(risk_signal(
            "edge_confidence",
            i64::from(min_confidence),
            1,
            if impacted.is_empty() {
                0
            } else if min_confidence >= 8_000 {
                10
            } else {
                4
            },
            "higher-confidence edges make the impact set more actionable",
        ));
        signals.push(risk_signal(
            "public_api_exposure",
            if public_api.0 { 1 } else { 0 },
            25,
            if public_api.0 { 25 } else { 0 },
            &public_api.1,
        ));
        signals.push(risk_signal(
            "test_coverage_presence",
            related_tests.len() as i64,
            -15,
            if related_tests.is_empty() { 18 } else { -10 },
            if related_tests.is_empty() {
                "no related tests found"
            } else {
                "related tests reduce change risk"
            },
        ));
        signals.push(risk_signal(
            "dependency_depth",
            max_distance as i64,
            6,
            (max_distance as i64 * 6).min(18),
            "deeper dependency paths raise coordination risk",
        ));
        signals.push(risk_signal(
            "cycle_presence",
            cycles_involved.len() as i64,
            20,
            if cycles_involved.is_empty() { 0 } else { 20 },
            "cycles make local reasoning less reliable",
        ));
        signals.push(risk_signal(
            "centrality",
            centrality
                .as_ref()
                .map(|metric| (metric.pagerank_score * 1_000_000.0) as i64)
                .unwrap_or_default(),
            15,
            centrality
                .as_ref()
                .map(centrality_risk_boost)
                .unwrap_or_default(),
            centrality
                .as_ref()
                .map(|_| "persisted PageRank/degree centrality contributes to risk")
                .unwrap_or("centrality snapshot not available"),
        ));

        let score = signals
            .iter()
            .map(|signal| signal.contribution)
            .sum::<i64>()
            .clamp(0, 100) as u16;
        let level = risk_level(score);
        let reasons = signals
            .iter()
            .filter(|signal| signal.contribution > 0)
            .map(|signal| format!("{}: {}", signal.name, signal.reason))
            .collect::<Vec<_>>();

        for signal in &signals {
            trace.trace.ranking_decisions.push(format!(
                "risk_signal {} value={} contribution={} reason={}",
                signal.name, signal.value, signal.contribution, signal.reason
            ));
        }
        for test in &related_tests {
            trace.trace.ranking_decisions.push(format!(
                "test_match {} confidence={} relation={}",
                test.symbol.symbol_id, test.confidence_bps, test.relation
            ));
        }
        if related_tests.is_empty() {
            trace
                .trace
                .warnings
                .push("missing tests: no related tests found".to_string());
        }

        Ok(ImpactRiskAssessment {
            score,
            level,
            reasons,
            signals,
            related_tests,
            impacted_files,
            dependency_paths,
            cycles_involved,
        })
    }

    fn impacted_file_paths(
        &self,
        scope: &QueryScope,
        seed: Option<&QuerySymbol>,
        impacted: &[TraversalStep],
    ) -> ContractResult<Vec<String>> {
        let mut paths = Vec::new();
        let mut seen = HashSet::new();
        for symbol in seed
            .into_iter()
            .chain(impacted.iter().map(|step| &step.symbol))
        {
            if !seen.insert(symbol.file_id.as_str().to_string()) {
                continue;
            }
            if let Some(file) = self.repository.get_file(scope, &symbol.file_id)? {
                paths.push(file.path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn related_tests(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        seed: Option<&QuerySymbol>,
        impacted: &[TraversalStep],
        trace: &mut QueryTraceBuilder,
    ) -> ContractResult<Vec<RelatedTestDto>> {
        let seed_name = seed.map(|symbol| symbol.name.as_str()).unwrap_or_default();
        let seed_file = seed.and_then(|symbol| {
            self.repository
                .get_file(scope, &symbol.file_id)
                .ok()
                .flatten()
        });
        let mut tests = Vec::new();

        for candidate in self.repository.list_symbols(scope, 2_000)? {
            let Some(file) = self.repository.get_file(scope, &candidate.file_id)? else {
                continue;
            };
            let path_is_test = is_test_path(&file.path);
            if candidate.kind != NodeKind::Test && !path_is_test {
                trace.trace.context_items_skipped.push(format!(
                    "{} skipped: not a test symbol or test path",
                    candidate.id.as_str()
                ));
                continue;
            }

            let mut confidence = 0_u16;
            let mut relation = Vec::new();
            if candidate.kind == NodeKind::Test {
                confidence = confidence.max(6_000);
                relation.push("symbol kind is Test");
            }
            if path_is_test {
                confidence = confidence.max(5_000);
                relation.push("file path looks like a test");
            }
            if name_similarity(&candidate.name, seed_name) {
                confidence = confidence.max(8_000);
                relation.push("test name matches impacted symbol");
            }
            if seed_file
                .as_ref()
                .is_some_and(|seed_file| same_module(&seed_file.path, &file.path))
            {
                confidence = confidence.max(6_500);
                relation.push("same module proximity");
            }
            if self.has_test_edge(scope, &candidate.id, symbol_id)? {
                confidence = confidence.max(9_000);
                relation.push("graph edge links test to symbol");
            }
            if impacted.iter().any(|step| {
                name_similarity(&candidate.name, &step.symbol.name)
                    || step.symbol.file_id == candidate.file_id
            }) {
                confidence = confidence.max(5_500);
                relation.push("matches impacted neighbor");
            }

            if confidence == 0 {
                trace.trace.context_items_skipped.push(format!(
                    "{} skipped: no confident relation to seed",
                    candidate.id.as_str()
                ));
                continue;
            }

            tests.push(RelatedTestDto {
                symbol: symbol_dto(&RankedSymbol {
                    symbol: candidate,
                    score: i64::from(confidence),
                    why: relation.join("; "),
                    graph_distance: 0,
                    ranking_decision: format!("test_confidence={confidence}"),
                }),
                confidence_bps: confidence,
                relation: relation.join("; "),
                direct: confidence >= 8_000,
            });
        }

        tests.sort_by(|left, right| {
            right
                .confidence_bps
                .cmp(&left.confidence_bps)
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        tests.truncate(16);
        Ok(tests)
    }

    fn has_test_edge(
        &self,
        scope: &QueryScope,
        test_symbol: &SymbolId,
        target_symbol: &SymbolId,
    ) -> ContractResult<bool> {
        let edges = self.repository.graph_neighbors(
            scope,
            test_symbol,
            GraphDirection::Outbound,
            &[EdgeKind::Calls, EdgeKind::References, EdgeKind::Tests],
            self.config.min_edge_confidence,
        )?;
        Ok(edges
            .iter()
            .any(|edge| edge.to_symbol.as_ref() == Some(target_symbol)))
    }

    pub fn dependency_path(
        &self,
        scope: &QueryScope,
        source: &SymbolId,
        target: &SymbolId,
        edge_filter: &[EdgeKind],
        max_depth: usize,
        min_confidence: u16,
    ) -> ContractResult<DependencyPath> {
        if source == target {
            let node = self.repository.get_symbol(scope, source)?;
            return Ok(DependencyPath {
                found: node.is_some(),
                nodes: node.into_iter().collect(),
                edges: Vec::new(),
                path_length: 0,
                confidence_summary: None,
            });
        }

        let bounded_depth = max_depth.min(self.config.retrieval.max_graph_depth as usize);
        let mut visited = HashSet::new();
        let mut queue = VecDeque::from([(source.clone(), Vec::<GraphNeighbor>::new())]);
        visited.insert(source.as_str().to_string());

        while let Some((current, path_edges)) = queue.pop_front() {
            if path_edges.len() >= bounded_depth {
                continue;
            }

            for edge in self.repository.graph_neighbors(
                scope,
                &current,
                GraphDirection::Outbound,
                edge_filter,
                min_confidence.max(self.config.min_edge_confidence),
            )? {
                let Some(next) = edge.to_symbol.clone() else {
                    continue;
                };
                if !visited.insert(next.as_str().to_string()) {
                    continue;
                }

                let mut next_path = path_edges.clone();
                next_path.push(edge);
                if &next == target {
                    return self.build_dependency_path(scope, source, target, next_path);
                }
                queue.push_back((next, next_path));
            }
        }

        Ok(DependencyPath {
            found: false,
            nodes: Vec::new(),
            edges: Vec::new(),
            path_length: 0,
            confidence_summary: None,
        })
    }

    fn build_dependency_path(
        &self,
        scope: &QueryScope,
        source: &SymbolId,
        target: &SymbolId,
        edges: Vec<GraphNeighbor>,
    ) -> ContractResult<DependencyPath> {
        let mut node_ids = Vec::from([source.clone()]);
        for edge in &edges {
            if let Some(to) = &edge.to_symbol {
                node_ids.push(to.clone());
            }
        }

        let mut nodes = Vec::new();
        for node_id in node_ids {
            if let Some(node) = self.repository.get_symbol(scope, &node_id)? {
                nodes.push(node);
            }
        }

        let confidence_summary = edges
            .iter()
            .map(|edge| usize::from(edge.confidence.basis_points()))
            .min()
            .map(|value| value as u16);
        let found = nodes.last().map(|node| &node.id) == Some(target);
        Ok(DependencyPath {
            found,
            path_length: edges.len(),
            nodes,
            edges,
            confidence_summary,
        })
    }

    pub fn detect_cycles(
        &self,
        scope: &QueryScope,
        edge_filter: &[EdgeKind],
        max_nodes: usize,
        min_confidence: u16,
    ) -> ContractResult<CycleDetectionResult> {
        let max_nodes = max_nodes.min(DEFAULT_CYCLE_NODE_LIMIT).max(1);
        let symbols = self.repository.list_symbols(scope, max_nodes)?;
        let symbol_ids = symbols
            .iter()
            .map(|symbol| symbol.id.clone())
            .collect::<Vec<_>>();
        let symbol_id_set = symbol_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect::<HashSet<_>>();
        let mut adjacency = Vec::new();

        for symbol_id in &symbol_ids {
            let neighbors = self.repository.graph_neighbors(
                scope,
                symbol_id,
                GraphDirection::Outbound,
                edge_filter,
                min_confidence.max(self.config.min_edge_confidence),
            )?;
            let targets = neighbors
                .into_iter()
                .filter_map(|edge| edge.to_symbol.map(|target| (target, edge.edge_kind)))
                .filter(|(target, _)| symbol_id_set.contains(target.as_str()))
                .collect::<Vec<_>>();
            adjacency.push((symbol_id.clone(), targets));
        }

        Ok(tarjan_cycles(&adjacency))
    }

    pub fn compute_centrality(&self, scope: &QueryScope) -> ContractResult<CentralitySnapshot> {
        self.compute_centrality_with_config(scope, &self.config.centrality)
    }

    pub fn compute_centrality_with_config(
        &self,
        scope: &QueryScope,
        config: &CentralityConfig,
    ) -> ContractResult<CentralitySnapshot> {
        let max_nodes = config.max_nodes.max(1);
        let symbols = self.repository.list_symbols(scope, max_nodes)?;
        let node_count = symbols.len();
        let symbol_ids = symbols
            .iter()
            .map(|symbol| symbol.id.clone())
            .collect::<Vec<_>>();
        let symbol_id_set = symbol_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect::<HashSet<_>>();
        let mut adjacency = HashMap::<String, Vec<String>>::new();
        let mut inbound = HashMap::<String, HashSet<String>>::new();

        for symbol in &symbol_ids {
            let neighbors = self.repository.graph_neighbors(
                scope,
                symbol,
                GraphDirection::Outbound,
                &config.edge_filter,
                self.config.min_edge_confidence,
            )?;
            for target in neighbors.into_iter().filter_map(|edge| edge.to_symbol) {
                if !symbol_id_set.contains(target.as_str()) {
                    continue;
                }
                adjacency
                    .entry(symbol.as_str().to_string())
                    .or_default()
                    .push(target.as_str().to_string());
                inbound
                    .entry(target.as_str().to_string())
                    .or_default()
                    .insert(symbol.as_str().to_string());
            }
            adjacency.entry(symbol.as_str().to_string()).or_default();
        }

        let cycles = self.detect_cycles(
            scope,
            &config.edge_filter,
            max_nodes,
            self.config.min_edge_confidence,
        )?;
        let cycle_members = cycles
            .cycles
            .iter()
            .flat_map(|cycle| cycle.node_ids.iter().map(|id| id.as_str().to_string()))
            .collect::<HashSet<_>>();
        let component_sizes = component_sizes(&symbol_ids, &adjacency);
        let pagerank = pagerank_scores(&symbol_ids, &adjacency, config);
        let denominator = node_count.saturating_sub(1).max(1) as f64;
        let calculated_at_unix_ms = stable_centrality_timestamp(scope, node_count);

        let mut metrics = Vec::new();
        for symbol in &symbol_ids {
            let key = symbol.as_str().to_string();
            let out_degree = adjacency.get(&key).map(Vec::len).unwrap_or_default();
            let in_degree = inbound.get(&key).map(HashSet::len).unwrap_or_default();
            metrics.push(CentralityMetric {
                symbol_id: key.clone(),
                in_degree,
                out_degree,
                fan_in: in_degree,
                fan_out: out_degree,
                degree_centrality: (in_degree + out_degree) as f64 / denominator,
                pagerank_score: pagerank.get(&key).copied().unwrap_or_default(),
                component_size: component_sizes.get(&key).copied(),
                is_cycle_member: cycle_members.contains(&key),
                algorithm_version: CENTRALITY_ALGORITHM_VERSION.to_string(),
                calculated_at_unix_ms,
            });
        }

        metrics.sort_by(|left, right| left.symbol_id.cmp(&right.symbol_id));
        let snapshot = CentralitySnapshot {
            project_id: scope.project_id.as_str().to_string(),
            branch_id: scope.branch_id.as_str().to_string(),
            algorithm_version: CENTRALITY_ALGORITHM_VERSION.to_string(),
            calculated_at_unix_ms,
            metrics,
        };
        self.repository
            .upsert_centrality_snapshot(scope, snapshot.clone())?;
        Ok(snapshot)
    }

    pub fn context_pack_for_symbols(
        &self,
        scope: &QueryScope,
        symbols: &[RankedSymbol],
        token_budget: usize,
    ) -> ContractResult<ContextPack> {
        let mut trace = QueryTraceBuilder::new(scope, "symbols", "get_context_pack");
        self.context_pack_for_symbols_with_trace(scope, symbols, token_budget, &mut trace)
    }

    pub fn context_pack_response_for_symbols(
        &self,
        scope: &QueryScope,
        symbols: &[RankedSymbol],
        token_budget: usize,
        include_trace: bool,
    ) -> ContractResult<ContextPackResponse> {
        let mut trace = QueryTraceBuilder::new(scope, "symbols", "get_context_pack");
        let pack =
            self.context_pack_for_symbols_with_trace(scope, symbols, token_budget, &mut trace)?;
        let trace = trace.finish();
        Ok(ContextPackResponse {
            items: pack.items.iter().map(context_item_dto).collect(),
            returned_tokens: pack.returned_tokens,
            token_budget: pack.token_budget,
            skipped_items: pack.skipped_items.clone(),
            truncation_reason: pack.truncation_reason.clone(),
            expansion_handles: pack.expansion_handles.clone(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    fn context_pack_for_symbols_with_trace(
        &self,
        scope: &QueryScope,
        symbols: &[RankedSymbol],
        token_budget: usize,
        trace: &mut QueryTraceBuilder,
    ) -> ContractResult<ContextPack> {
        let mut items = Vec::new();
        let mut handles = Vec::new();
        let mut seen_symbols = HashSet::new();
        let mut seen_snippets = HashSet::new();
        let mut skipped_items = Vec::new();
        let mut returned_tokens = 0;
        let mut truncation_reason = None;
        let mut candidates = Vec::new();
        let mut file_counts = std::collections::HashMap::<String, usize>::new();
        let mut module_counts = std::collections::HashMap::<String, usize>::new();

        for ranked in symbols {
            let key = ranked.symbol.id.as_str().to_string();
            if !seen_symbols.insert(key.clone()) {
                skipped_items.push(format!("{key}: duplicate symbol"));
                trace
                    .trace
                    .context_items_skipped
                    .push(format!("{key}: duplicate symbol"));
                continue;
            }

            let Some(file) = self.repository.get_file(scope, &ranked.symbol.file_id)? else {
                skipped_items.push(format!("{key}: missing file"));
                trace
                    .trace
                    .context_items_skipped
                    .push(format!("{key}: missing file"));
                continue;
            };
            let snippet = compact_snippet(&ranked.symbol.snippet);
            if !seen_snippets.insert(snippet.clone()) {
                skipped_items.push(format!("{key}: duplicate snippet"));
                trace
                    .trace
                    .context_items_skipped
                    .push(format!("{key}: duplicate snippet"));
                continue;
            }
            let estimated_tokens = estimate_tokens(&snippet) + estimate_tokens(&ranked.symbol.name);
            let module = module_key(&file.path);
            *file_counts.entry(file.path.clone()).or_default() += 1;
            *module_counts.entry(module).or_default() += 1;
            candidates.push(PackCandidate {
                ranked: ranked.clone(),
                file_path: file.path,
                snippet,
                estimated_tokens,
                value_per_token: 0.0,
                penalties: Vec::new(),
            });
        }

        for candidate in &mut candidates {
            let mut penalty = 1.0_f64;
            if file_counts
                .get(&candidate.file_path)
                .copied()
                .unwrap_or_default()
                > 1
            {
                penalty *= 0.85;
                candidate
                    .penalties
                    .push("duplicate_file_penalty".to_string());
            }
            if module_counts
                .get(&module_key(&candidate.file_path))
                .copied()
                .unwrap_or_default()
                > 2
            {
                penalty *= 0.90;
                candidate
                    .penalties
                    .push("same_module_saturation_penalty".to_string());
            }
            candidate.value_per_token = (candidate.ranked.score.max(1) as f64 * penalty)
                / candidate.estimated_tokens as f64;
        }

        candidates.sort_by(|left, right| {
            right
                .value_per_token
                .total_cmp(&left.value_per_token)
                .then_with(|| right.ranked.score.cmp(&left.ranked.score))
                .then_with(|| left.ranked.symbol.name.cmp(&right.ranked.symbol.name))
        });

        for candidate in candidates {
            let key = candidate.ranked.symbol.id.as_str().to_string();
            let estimated_tokens = candidate.estimated_tokens;
            if estimated_tokens > token_budget || returned_tokens + estimated_tokens > token_budget
            {
                let reason = format!("{key}: token budget exceeded");
                skipped_items.push(reason.clone());
                trace.trace.context_items_skipped.push(reason.clone());
                truncation_reason = Some("token budget exhausted".to_string());
                continue;
            }

            let handle = format!("symbol:{}", candidate.ranked.symbol.id.as_str());
            handles.push(handle.clone());
            returned_tokens += estimated_tokens;
            trace.trace.context_items_selected.push(format!(
                "{} value_per_token={:.3} penalties={}",
                candidate.ranked.symbol.id.as_str(),
                candidate.value_per_token,
                if candidate.penalties.is_empty() {
                    "none".to_string()
                } else {
                    candidate.penalties.join(",")
                }
            ));
            items.push(ContextItem {
                file_id: candidate.ranked.symbol.file_id.clone(),
                symbol_id: Some(candidate.ranked.symbol.id.clone()),
                title: format!("{} ({})", candidate.ranked.symbol.name, candidate.file_path),
                snippet: candidate.snippet,
                why: format!(
                    "{}; value_per_token={:.3}; penalties={}",
                    candidate.ranked.why,
                    candidate.value_per_token,
                    if candidate.penalties.is_empty() {
                        "none".to_string()
                    } else {
                        candidate.penalties.join(",")
                    }
                ),
                source_provenance: "sqlite:symbols.snippet".to_string(),
                estimated_tokens,
                expansion_handle: handle,
            });
        }

        trace.trace.token_budget = token_budget;
        trace.trace.token_budget_used = returned_tokens;
        trace.trace.truncation_reason = truncation_reason.clone();
        let savings = QuerySavingsEstimate {
            tool_call_id: None,
            returned_tokens,
            avoided_file_reads: items.len(),
            avoided_search_calls: 1,
        };
        trace.trace.token_savings_estimate = Some(QuerySavingsEstimateDto {
            returned_tokens: savings.returned_tokens,
            avoided_file_reads: savings.avoided_file_reads,
            avoided_search_calls: savings.avoided_search_calls,
            estimated_tokens_saved: savings.avoided_file_reads.saturating_mul(800),
        });
        if let Err(error) = self.record_savings(savings) {
            trace.trace.warnings.push(format!(
                "token savings ledger write failed: {}",
                error.message
            ));
        }

        Ok(ContextPack {
            items,
            returned_tokens,
            token_budget,
            skipped_items,
            truncation_reason,
            expansion_handles: handles,
        })
    }

    pub fn context_pack_for_query(
        &self,
        scope: &QueryScope,
        query: &str,
        token_budget: usize,
    ) -> ContractResult<ContextPack> {
        let mut ranked = self.find_symbol(scope, query)?;
        ranked.extend(self.search_code(scope, query, DEFAULT_FTS_LIMIT)?);
        dedupe_ranked(&mut ranked);
        self.context_pack_for_symbols(scope, &ranked, token_budget)
    }

    pub fn context_pack_response_for_query(
        &self,
        scope: &QueryScope,
        query: &str,
        token_budget: usize,
        include_trace: bool,
    ) -> ContractResult<ContextPackResponse> {
        let mut trace = QueryTraceBuilder::new(scope, query, "get_context_pack");
        let mut ranked = self.find_symbol(scope, query)?;
        for symbol in &ranked {
            trace
                .trace
                .exact_symbol_hits
                .push(symbol.symbol.id.as_str().to_string());
        }
        let fts = self.search_code(scope, query, DEFAULT_FTS_LIMIT)?;
        for symbol in &fts {
            trace
                .trace
                .fts_hits
                .push(symbol.symbol.id.as_str().to_string());
        }
        ranked.extend(fts);
        dedupe_ranked(&mut ranked);
        for symbol in &ranked {
            trace.trace.ranking_decisions.push(format!(
                "{} score={} reason={} decision={}",
                symbol.symbol.name, symbol.score, symbol.why, symbol.ranking_decision
            ));
        }
        let pack =
            self.context_pack_for_symbols_with_trace(scope, &ranked, token_budget, &mut trace)?;
        let trace = trace.finish();
        Ok(ContextPackResponse {
            items: pack.items.iter().map(context_item_dto).collect(),
            returned_tokens: pack.returned_tokens,
            token_budget: pack.token_budget,
            skipped_items: pack.skipped_items.clone(),
            truncation_reason: pack.truncation_reason.clone(),
            expansion_handles: pack.expansion_handles.clone(),
            trace_id: trace.trace_id.clone(),
            trace: include_trace.then_some(trace),
        })
    }

    fn traverse(
        &self,
        scope: &QueryScope,
        start: &SymbolId,
        direction: GraphDirection,
        edge_filter: &[EdgeKind],
        max_depth: usize,
    ) -> ContractResult<Vec<TraversalStep>> {
        if max_depth == 0 {
            return Ok(Vec::new());
        }

        let bounded_depth = max_depth.min(self.config.retrieval.max_graph_depth as usize);
        let mut visited = HashSet::new();
        let mut queue = VecDeque::from([(start.clone(), 0_usize)]);
        let mut steps = Vec::new();
        visited.insert(start.as_str().to_string());

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= bounded_depth || steps.len() >= DEFAULT_GRAPH_LIMIT {
                continue;
            }

            let neighbors = self.repository.graph_neighbors(
                scope,
                &current,
                direction,
                edge_filter,
                self.config.min_edge_confidence,
            )?;

            for neighbor in neighbors {
                let next = next_symbol_id(&neighbor, &current, direction);
                let Some(next) = next else {
                    continue;
                };
                if !visited.insert(next.as_str().to_string()) {
                    continue;
                }

                if let Some(symbol) = self.repository.get_symbol(scope, &next)? {
                    let distance = depth + 1;
                    steps.push(TraversalStep {
                        symbol,
                        via: neighbor,
                        distance,
                    });
                    queue.push_back((next, distance));
                }
            }
        }

        let mut centrality = HashMap::new();
        for step in &steps {
            if let Some(metric) = self
                .repository
                .get_centrality_metric(scope, &step.symbol.id)?
            {
                centrality.insert(step.symbol.id.as_str().to_string(), metric);
            }
        }

        steps.sort_by(|left, right| {
            let left_centrality = centrality
                .get(left.symbol.id.as_str())
                .map(centrality_sort_score)
                .unwrap_or_default();
            let right_centrality = centrality
                .get(right.symbol.id.as_str())
                .map(centrality_sort_score)
                .unwrap_or_default();
            left.distance
                .cmp(&right.distance)
                .then_with(|| {
                    right
                        .via
                        .confidence
                        .basis_points()
                        .cmp(&left.via.confidence.basis_points())
                })
                .then_with(|| right_centrality.cmp(&left_centrality))
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        Ok(steps)
    }

    fn apply_centrality_boost(
        &self,
        scope: &QueryScope,
        symbols: &mut [RankedSymbol],
    ) -> ContractResult<()> {
        for ranked in symbols {
            let Some(metric) = self
                .repository
                .get_centrality_metric(scope, &ranked.symbol.id)?
            else {
                continue;
            };
            let boost = centrality_rank_boost(&metric);
            ranked.score += boost;
            ranked.why.push_str(&format!("; centrality boost {boost}"));
            ranked.ranking_decision.push_str(&format!(
                " centrality_score={} pagerank={:.6} degree_centrality={:.6}",
                boost, metric.pagerank_score, metric.degree_centrality
            ));
        }
        Ok(())
    }

    fn record_savings(&self, estimate: QuerySavingsEstimate) -> ContractResult<()> {
        self.repository.record_savings(TokenSavingsRecord {
            tool_call_id: estimate.tool_call_id,
            estimated_tokens_saved: estimate.avoided_file_reads.saturating_mul(800),
            returned_tokens: estimate.returned_tokens,
            avoided_file_reads: estimate.avoided_file_reads,
            avoided_search_calls: estimate.avoided_search_calls,
        })
    }
}

impl<R> LocalQueryEngine<R>
where
    R: QueryRepository + VectorStore,
{
    pub fn hybrid_search_response(
        &self,
        request: HybridSearchRequest,
    ) -> ContractResult<HybridSearchResponse> {
        HybridSearchEngine::new(&self.repository, &self.repository).search(request)
    }
}

impl<R> QueryEngine for LocalQueryEngine<R>
where
    R: QueryRepository + TokenSavingsRepository + CentralityRepository,
{
    fn execute(&self, request: QueryRequest) -> ContractResult<QueryResult> {
        Err(ContractError::new(format!(
            "QueryEngine::execute requires a QueryScope before MCP tool wiring: {} / {}",
            request.text, request.token_budget
        )))
    }
}

fn next_symbol_id(
    neighbor: &GraphNeighbor,
    current: &SymbolId,
    direction: GraphDirection,
) -> Option<SymbolId> {
    match direction {
        GraphDirection::Outbound => neighbor.to_symbol.clone(),
        GraphDirection::Inbound => neighbor.from_symbol.clone(),
        GraphDirection::Both => {
            if neighbor.from_symbol.as_ref() == Some(current) {
                neighbor.to_symbol.clone()
            } else if neighbor.to_symbol.as_ref() == Some(current) {
                neighbor.from_symbol.clone()
            } else {
                None
            }
        }
    }
}

pub fn classify_query_intent(query: &str) -> QueryIntent {
    let normalized = query.to_ascii_lowercase();
    if normalized.contains("caller") || normalized.contains("who calls") {
        QueryIntent::CallerLookup
    } else if normalized.contains("callee") || normalized.contains("calls from") {
        QueryIntent::CalleeLookup
    } else if normalized.contains("dependency")
        || normalized.contains("path")
        || normalized.contains("trace")
    {
        QueryIntent::DependencyTrace
    } else if normalized.contains("impact") || normalized.contains("affected") {
        QueryIntent::ImpactAnalysis
    } else if normalized.contains("context") || normalized.contains("pack") {
        QueryIntent::ContextPack
    } else if normalized.contains("test") || normalized.starts_with("#[test]") {
        QueryIntent::TestSearch
    } else if normalized.contains("explain") || normalized.contains("what is") {
        QueryIntent::Explanation
    } else if query
        .chars()
        .all(|character| character.is_alphanumeric() || character == '_' || character == ':')
    {
        QueryIntent::SymbolLookup
    } else {
        QueryIntent::CodeSearch
    }
}

fn ranking_profile(intent: QueryIntent, base: RankingWeights) -> RankingProfile {
    match intent {
        QueryIntent::SymbolLookup => RankingProfile {
            exact_match: i64::from(base.exact_symbol) * 12,
            bm25: i64::from(base.lexical_bm25) * 3,
            graph_distance: i64::from(base.graph_proximity),
            edge_confidence: 1,
            symbol_kind: 80,
            provenance: 20,
        },
        QueryIntent::CodeSearch => RankingProfile {
            exact_match: i64::from(base.exact_symbol) * 4,
            bm25: i64::from(base.lexical_bm25) * 10,
            graph_distance: i64::from(base.graph_proximity),
            edge_confidence: 1,
            symbol_kind: 20,
            provenance: 20,
        },
        QueryIntent::TestSearch => RankingProfile {
            exact_match: i64::from(base.exact_symbol) * 4,
            bm25: i64::from(base.lexical_bm25) * 7,
            graph_distance: i64::from(base.graph_proximity),
            edge_confidence: 1,
            symbol_kind: 140,
            provenance: 20,
        },
        QueryIntent::CallerLookup | QueryIntent::CalleeLookup | QueryIntent::DependencyTrace => {
            RankingProfile {
                exact_match: i64::from(base.exact_symbol) * 3,
                bm25: i64::from(base.lexical_bm25) * 2,
                graph_distance: i64::from(base.graph_proximity) * 4,
                edge_confidence: 2,
                symbol_kind: 30,
                provenance: 80,
            }
        }
        QueryIntent::ImpactAnalysis => RankingProfile {
            exact_match: i64::from(base.exact_symbol) * 3,
            bm25: i64::from(base.lexical_bm25) * 2,
            graph_distance: i64::from(base.graph_proximity) * 3,
            edge_confidence: 2,
            symbol_kind: 60,
            provenance: 80,
        },
        QueryIntent::ContextPack | QueryIntent::Explanation => RankingProfile {
            exact_match: i64::from(base.exact_symbol) * 6,
            bm25: i64::from(base.lexical_bm25) * 6,
            graph_distance: i64::from(base.graph_proximity) * 2,
            edge_confidence: 1,
            symbol_kind: 50,
            provenance: 40,
        },
    }
}

fn rank_symbol(
    symbol: QuerySymbol,
    query: &str,
    intent: QueryIntent,
    weights: RankingWeights,
    bm25: Option<f32>,
    graph_distance: usize,
) -> RankedSymbol {
    let profile = ranking_profile(intent, weights);
    let exact_match_score = if symbol.name == query {
        profile.exact_match
    } else if symbol.name.contains(query) {
        profile.exact_match / 2
    } else {
        0
    };
    let bm25_score = bm25
        .map(|score| lexical_score(score, profile.bm25 as u16))
        .unwrap_or_default();
    let graph_distance_score = if graph_distance == 0 {
        0
    } else {
        profile.graph_distance / graph_distance as i64
    };
    let symbol_kind_score = symbol_kind_score(symbol.kind, intent, profile.symbol_kind);
    let provenance_score = profile.provenance;
    let score = exact_match_score
        + bm25_score
        + graph_distance_score
        + symbol_kind_score
        + provenance_score;
    let ranking_decision = format!(
        "intent={} exact_match_score={} bm25_score={} graph_distance_score={} edge_confidence_score=0 symbol_kind_score={} provenance_score={}",
        intent.as_str(),
        exact_match_score,
        bm25_score,
        graph_distance_score,
        symbol_kind_score,
        provenance_score
    );

    RankedSymbol {
        symbol,
        score,
        why: match bm25 {
            Some(_) => format!("{} lexical match", intent.as_str()),
            None => format!("{} exact symbol match", intent.as_str()),
        },
        graph_distance,
        ranking_decision,
    }
}

fn symbol_kind_score(kind: NodeKind, intent: QueryIntent, weight: i64) -> i64 {
    match (intent, kind) {
        (QueryIntent::TestSearch, NodeKind::Test) => weight,
        (QueryIntent::SymbolLookup, NodeKind::Function | NodeKind::Method | NodeKind::Struct) => {
            weight
        }
        (
            QueryIntent::CallerLookup | QueryIntent::CalleeLookup,
            NodeKind::Function | NodeKind::Method,
        ) => weight,
        (_, NodeKind::Function | NodeKind::Method) => weight / 2,
        _ => 0,
    }
}

fn stable_trace_id(scope: &QueryScope, intent: &str, input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    scope.project_id.as_str().hash(&mut hasher);
    scope.branch_id.as_str().hash(&mut hasher);
    intent.hash(&mut hasher);
    input.hash(&mut hasher);
    format!("trace-{:x}", hasher.finish())
}

fn symbol_dto(ranked: &RankedSymbol) -> SymbolDto {
    SymbolDto {
        symbol_id: ranked.symbol.id.as_str().to_string(),
        file_id: ranked.symbol.file_id.as_str().to_string(),
        name: ranked.symbol.name.clone(),
        kind: format!("{:?}", ranked.symbol.kind),
        start_line: ranked.symbol.start_line,
        end_line: ranked.symbol.end_line,
        visibility: ranked.symbol.visibility.clone(),
        score: ranked.score,
        why: ranked.why.clone(),
    }
}

fn traversal_step_dto(step: &TraversalStep, direction: &str) -> TraversalStepDto {
    TraversalStepDto {
        symbol: symbol_dto(&RankedSymbol {
            symbol: step.symbol.clone(),
            score: traversal_score(step),
            why: "graph traversal".to_string(),
            graph_distance: step.distance,
            ranking_decision: format!(
                "graph_distance_score={} edge_confidence_score={}",
                -((step.distance as i64) * 10),
                step.via.confidence.basis_points()
            ),
        }),
        edge_id: step.via.edge_id.as_str().to_string(),
        edge_kind: format!("{:?}", step.via.edge_kind),
        direction: direction.to_string(),
        distance: step.distance,
        confidence_bps: step.via.confidence.basis_points(),
        provenance: format!("{:?}", step.via.provenance),
    }
}

fn context_item_dto(item: &ContextItem) -> ContextItemDto {
    ContextItemDto {
        file_id: item.file_id.as_str().to_string(),
        symbol_id: item.symbol_id.as_ref().map(|id| id.as_str().to_string()),
        title: item.title.clone(),
        snippet: item.snippet.clone(),
        why: item.why.clone(),
        source_provenance: item.source_provenance.clone(),
        estimated_tokens: item.estimated_tokens,
        expansion_handle: item.expansion_handle.clone(),
    }
}

fn record_steps(trace: &mut QueryTraceBuilder, steps: &[TraversalStep]) {
    for step in steps {
        trace.trace.graph_traversal_steps.push(format!(
            "{} via {:?} distance={} confidence={}",
            step.symbol.id.as_str(),
            step.via.edge_kind,
            step.distance,
            step.via.confidence.basis_points()
        ));
        trace.trace.ranking_decisions.push(format!(
            "{} graph_distance={} confidence={}",
            step.symbol.name,
            step.distance,
            step.via.confidence.basis_points()
        ));
    }
}

fn traversal_score(step: &TraversalStep) -> i64 {
    i64::from(step.via.confidence.basis_points()) + provenance_score(step.via.provenance)
        - step.distance as i64
}

fn provenance_score(provenance: EdgeProvenance) -> i64 {
    match provenance {
        EdgeProvenance::Ast => 100,
        EdgeProvenance::ImportAnalysis => 80,
        EdgeProvenance::TextHeuristic => 40,
        EdgeProvenance::SemanticSimilarity => 0,
        EdgeProvenance::UserRecorded => 90,
    }
}

fn pagerank_scores(
    nodes: &[SymbolId],
    adjacency: &HashMap<String, Vec<String>>,
    config: &CentralityConfig,
) -> HashMap<String, f64> {
    if nodes.is_empty() {
        return HashMap::new();
    }

    let node_keys = nodes
        .iter()
        .map(|node| node.as_str().to_string())
        .collect::<Vec<_>>();
    let node_count = node_keys.len() as f64;
    let damping = config.damping_factor.clamp(0.0, 1.0);
    let base = (1.0 - damping) / node_count;
    let mut scores = node_keys
        .iter()
        .map(|node| (node.clone(), 1.0 / node_count))
        .collect::<HashMap<_, _>>();

    for _ in 0..config.iterations {
        let mut next = node_keys
            .iter()
            .map(|node| (node.clone(), base))
            .collect::<HashMap<_, _>>();
        let mut dangling = 0.0;

        for node in &node_keys {
            let score = scores.get(node).copied().unwrap_or_default();
            let outgoing = adjacency.get(node).map(Vec::as_slice).unwrap_or(&[]);
            if outgoing.is_empty() {
                dangling += score;
                continue;
            }
            let contribution = damping * score / outgoing.len() as f64;
            for target in outgoing {
                if let Some(value) = next.get_mut(target) {
                    *value += contribution;
                }
            }
        }

        if dangling > 0.0 {
            let contribution = damping * dangling / node_count;
            for value in next.values_mut() {
                *value += contribution;
            }
        }

        let delta = node_keys
            .iter()
            .map(|node| {
                (next.get(node).copied().unwrap_or_default()
                    - scores.get(node).copied().unwrap_or_default())
                .abs()
            })
            .sum::<f64>();
        scores = next;
        if delta <= config.convergence_threshold {
            break;
        }
    }

    scores
}

fn component_sizes(
    nodes: &[SymbolId],
    adjacency: &HashMap<String, Vec<String>>,
) -> HashMap<String, usize> {
    let node_set = nodes
        .iter()
        .map(|node| node.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut undirected = HashMap::<String, Vec<String>>::new();
    for node in &node_set {
        undirected.entry(node.clone()).or_default();
    }
    for (source, targets) in adjacency {
        for target in targets {
            if !node_set.contains(target) {
                continue;
            }
            undirected
                .entry(source.clone())
                .or_default()
                .push(target.clone());
            undirected
                .entry(target.clone())
                .or_default()
                .push(source.clone());
        }
    }

    let mut sizes = HashMap::new();
    let mut visited = HashSet::new();
    for node in &node_set {
        if !visited.insert(node.clone()) {
            continue;
        }
        let mut stack = vec![node.clone()];
        let mut component = Vec::new();
        while let Some(current) = stack.pop() {
            component.push(current.clone());
            for next in undirected.get(&current).into_iter().flatten() {
                if visited.insert(next.clone()) {
                    stack.push(next.clone());
                }
            }
        }
        let size = component.len();
        for member in component {
            sizes.insert(member, size);
        }
    }
    sizes
}

fn centrality_rank_boost(metric: &CentralityMetric) -> i64 {
    ((metric.pagerank_score * 500.0) as i64
        + (metric.degree_centrality * 40.0) as i64
        + metric.in_degree.min(10) as i64)
        .min(80)
}

fn centrality_risk_boost(metric: &CentralityMetric) -> i64 {
    ((metric.pagerank_score * 120.0) as i64
        + (metric.degree_centrality * 20.0) as i64
        + metric.fan_in.min(10) as i64)
        .min(18)
}

fn centrality_sort_score(metric: &CentralityMetric) -> i64 {
    ((metric.pagerank_score * 1_000_000.0) as i64)
        + ((metric.degree_centrality * 10_000.0) as i64)
        + metric.in_degree as i64
}

fn stable_centrality_timestamp(scope: &QueryScope, node_count: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    scope.project_id.as_str().hash(&mut hasher);
    scope.branch_id.as_str().hash(&mut hasher);
    CENTRALITY_ALGORITHM_VERSION.hash(&mut hasher);
    node_count.hash(&mut hasher);
    hasher.finish()
}

fn risk_signal(
    name: &str,
    value: i64,
    weight: i64,
    contribution: i64,
    reason: &str,
) -> ImpactRiskSignalDto {
    ImpactRiskSignalDto {
        name: name.to_string(),
        value,
        weight,
        contribution,
        reason: reason.to_string(),
    }
}

fn risk_level(score: u16) -> ImpactRiskLevel {
    match score {
        0..=24 => ImpactRiskLevel::Low,
        25..=49 => ImpactRiskLevel::Medium,
        50..=74 => ImpactRiskLevel::High,
        _ => ImpactRiskLevel::Critical,
    }
}

fn public_api_exposure(symbol: &QuerySymbol) -> (bool, String) {
    if symbol
        .visibility
        .as_deref()
        .is_some_and(|visibility| matches!(visibility, "pub" | "public"))
    {
        return (true, "symbol visibility is public".to_string());
    }
    if matches!(
        symbol.kind,
        NodeKind::Route | NodeKind::Endpoint | NodeKind::Interface
    ) {
        return (true, "symbol kind is externally addressable".to_string());
    }
    if symbol.kind == NodeKind::Method
        && symbol
            .snippet
            .lines()
            .next()
            .is_some_and(|line| line.contains("pub "))
    {
        return (true, "method declaration appears exported".to_string());
    }
    (false, "symbol is not visibly public".to_string())
}

fn is_test_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.contains("/test/")
        || normalized.contains("/tests/")
        || normalized.contains("/spec/")
        || normalized.contains("/specs/")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with("_spec.rs")
}

fn name_similarity(test_name: &str, target_name: &str) -> bool {
    if target_name.is_empty() {
        return false;
    }
    let test = test_name.to_ascii_lowercase();
    let target = target_name.to_ascii_lowercase();
    test == target
        || test.contains(&target)
        || target
            .split('_')
            .filter(|part| part.len() > 2)
            .any(|part| test.contains(part))
}

fn same_module(left: &str, right: &str) -> bool {
    module_key(left) == module_key(right)
}

fn lexical_score(raw_bm25: f32, weight: u16) -> i64 {
    let magnitude = if raw_bm25.is_sign_negative() {
        -raw_bm25
    } else {
        raw_bm25
    };
    i64::from(weight) + (magnitude * 100.0) as i64
}

fn compact_snippet(snippet: &str) -> String {
    const MAX_CHARS: usize = 600;
    let mut compact = snippet
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if compact.len() > MAX_CHARS {
        compact.truncate(MAX_CHARS);
    }
    compact
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4).max(1)
}

fn dedupe_ranked(symbols: &mut Vec<RankedSymbol>) {
    symbols.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.graph_distance.cmp(&right.graph_distance))
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    let mut seen = HashSet::new();
    symbols.retain(|ranked| seen.insert(ranked.symbol.id.as_str().to_string()));
}

fn dedupe_steps(steps: &mut Vec<TraversalStep>) {
    steps.sort_by(|left, right| {
        left.distance
            .cmp(&right.distance)
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    let mut seen = HashSet::new();
    steps.retain(|step| seen.insert(step.symbol.id.as_str().to_string()));
}

fn module_key(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(module, _)| module.to_string())
        .unwrap_or_default()
}

fn tarjan_cycles(adjacency: &[(SymbolId, Vec<(SymbolId, EdgeKind)>)]) -> CycleDetectionResult {
    struct TarjanState<'a> {
        adjacency: &'a [(SymbolId, Vec<(SymbolId, EdgeKind)>)],
        index: usize,
        stack: Vec<SymbolId>,
        on_stack: HashSet<String>,
        indexes: std::collections::HashMap<String, usize>,
        lowlinks: std::collections::HashMap<String, usize>,
        cycles: Vec<CycleGroup>,
    }

    impl TarjanState<'_> {
        fn strong_connect(&mut self, node: &SymbolId) {
            let node_key = node.as_str().to_string();
            self.indexes.insert(node_key.clone(), self.index);
            self.lowlinks.insert(node_key.clone(), self.index);
            self.index += 1;
            self.stack.push(node.clone());
            self.on_stack.insert(node_key.clone());

            let neighbors = self.neighbors(node).to_vec();
            for (target, _) in &neighbors {
                let target_key = target.as_str().to_string();
                if !self.indexes.contains_key(&target_key) {
                    self.strong_connect(target);
                    let low_node = self.lowlinks[&node_key];
                    let low_target = self.lowlinks[&target_key];
                    self.lowlinks
                        .insert(node_key.clone(), low_node.min(low_target));
                } else if self.on_stack.contains(&target_key) {
                    let low_node = self.lowlinks[&node_key];
                    let target_index = self.indexes[&target_key];
                    self.lowlinks
                        .insert(node_key.clone(), low_node.min(target_index));
                }
            }

            if self.lowlinks[&node_key] == self.indexes[&node_key] {
                let mut component = Vec::new();
                while let Some(value) = self.stack.pop() {
                    self.on_stack.remove(value.as_str());
                    let done = value.as_str() == node.as_str();
                    component.push(value);
                    if done {
                        break;
                    }
                }

                if component.len() > 1 || self.has_self_loop(&component[0]) {
                    let component_keys = component
                        .iter()
                        .map(|id| id.as_str().to_string())
                        .collect::<HashSet<_>>();
                    let mut edge_types = Vec::new();
                    for node in &component {
                        for (target, edge_kind) in self.neighbors(node) {
                            if component_keys.contains(target.as_str())
                                && !edge_types.contains(edge_kind)
                            {
                                edge_types.push(*edge_kind);
                            }
                        }
                    }
                    component.sort_by(|left, right| left.as_str().cmp(right.as_str()));
                    self.cycles.push(CycleGroup {
                        node_ids: component,
                        edge_types,
                    });
                }
            }
        }

        fn neighbors(&self, node: &SymbolId) -> &[(SymbolId, EdgeKind)] {
            self.adjacency
                .iter()
                .find(|(candidate, _)| candidate == node)
                .map(|(_, edges)| edges.as_slice())
                .unwrap_or(&[])
        }

        fn has_self_loop(&self, node: &SymbolId) -> bool {
            self.neighbors(node)
                .iter()
                .any(|(target, _)| target.as_str() == node.as_str())
        }
    }

    let mut state = TarjanState {
        adjacency,
        index: 0,
        stack: Vec::new(),
        on_stack: HashSet::new(),
        indexes: std::collections::HashMap::new(),
        lowlinks: std::collections::HashMap::new(),
        cycles: Vec::new(),
    };

    for (node, _) in adjacency {
        if !state.indexes.contains_key(node.as_str()) {
            state.strong_connect(node);
        }
    }

    let summary_count = state.cycles.len();
    CycleDetectionResult {
        cycles: state.cycles,
        scanned_nodes: adjacency.len(),
        summary_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{
        BranchId, ContractError, ContractResult, DomainEvent, EventBus, FtsSearchHit,
        GraphNeighbor, IndexJob, Indexer, ProjectId, QueryFile, TokenSavingsRecord,
    };
    use b3_indexer::{IndexerConfig, LocalIndexer, RustLanguagePack};
    use b3_storage::SqliteStorage;
    use std::fs;
    use tempfile::tempdir;

    #[derive(Default)]
    struct TestEventBus;

    impl EventBus for TestEventBus {
        fn publish(&self, _event: DomainEvent) -> ContractResult<()> {
            Ok(())
        }
    }

    struct QueryFixture {
        storage: SqliteStorage,
        scope: QueryScope,
    }

    struct FailingLedgerRepo<'a>(&'a SqliteStorage);

    impl QueryRepository for FailingLedgerRepo<'_> {
        fn list_symbols(
            &self,
            scope: &QueryScope,
            limit: usize,
        ) -> ContractResult<Vec<QuerySymbol>> {
            self.0.list_symbols(scope, limit)
        }

        fn find_symbols(&self, scope: &QueryScope, name: &str) -> ContractResult<Vec<QuerySymbol>> {
            self.0.find_symbols(scope, name)
        }

        fn get_symbol(
            &self,
            scope: &QueryScope,
            symbol_id: &SymbolId,
        ) -> ContractResult<Option<QuerySymbol>> {
            self.0.get_symbol(scope, symbol_id)
        }

        fn get_file(
            &self,
            scope: &QueryScope,
            file_id: &b3_core::FileId,
        ) -> ContractResult<Option<QueryFile>> {
            self.0.get_file(scope, file_id)
        }

        fn fts_search(
            &self,
            scope: &QueryScope,
            query: &str,
            limit: usize,
        ) -> ContractResult<Vec<FtsSearchHit>> {
            self.0.fts_search(scope, query, limit)
        }

        fn graph_neighbors(
            &self,
            scope: &QueryScope,
            symbol_id: &SymbolId,
            direction: GraphDirection,
            edge_filter: &[EdgeKind],
            min_confidence: u16,
        ) -> ContractResult<Vec<GraphNeighbor>> {
            self.0
                .graph_neighbors(scope, symbol_id, direction, edge_filter, min_confidence)
        }
    }

    impl TokenSavingsRepository for FailingLedgerRepo<'_> {
        fn record_savings(&self, _record: TokenSavingsRecord) -> ContractResult<()> {
            Err(ContractError::new("ledger unavailable"))
        }
    }

    impl CentralityRepository for FailingLedgerRepo<'_> {
        fn get_centrality_metric(
            &self,
            scope: &QueryScope,
            symbol_id: &SymbolId,
        ) -> ContractResult<Option<CentralityMetric>> {
            self.0.get_centrality_metric(scope, symbol_id)
        }

        fn upsert_centrality_snapshot(
            &self,
            scope: &QueryScope,
            snapshot: CentralitySnapshot,
        ) -> ContractResult<()> {
            self.0.upsert_centrality_snapshot(scope, snapshot)
        }
    }

    fn fixture(branch_name: &str, source: &str) -> QueryFixture {
        let dir = tempdir().expect("temp dir").keep();
        let root = dir.join("project");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join("src").join("lib.rs"), source).expect("write rust");

        let storage = SqliteStorage::open(dir.join("b3.db")).expect("open storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new(branch_name);
        let indexer = LocalIndexer::new(
            RustLanguagePack,
            &storage,
            TestEventBus,
            IndexerConfig {
                branch_id: branch_id.clone(),
                ..IndexerConfig::default()
            },
        );
        indexer
            .index(IndexJob {
                project_id: project_id.clone(),
                root_path: root.to_string_lossy().to_string(),
            })
            .expect("index");

        QueryFixture {
            storage,
            scope: QueryScope::new(project_id, branch_id),
        }
    }

    fn sample_source() -> &'static str {
        r#"
            pub struct Runner;

            impl Runner {
                pub fn run(&self) {
                    helper();
                }
            }

            pub fn entry() {
                helper();
            }

            fn helper() {}
        "#
    }

    #[test]
    fn classifies_query_intents_with_rules() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());

        assert_eq!(engine.classify_intent("helper"), QueryIntent::SymbolLookup);
        assert_eq!(
            engine.classify_intent("who calls helper"),
            QueryIntent::CallerLookup
        );
        assert_eq!(
            engine.classify_intent("dependency path from a to b"),
            QueryIntent::DependencyTrace
        );
        assert_eq!(
            engine.classify_intent("find tests for helper"),
            QueryIntent::TestSearch
        );
    }

    #[test]
    fn adaptive_ranking_changes_scores_by_intent() {
        let symbol = QuerySymbol {
            id: SymbolId::new("symbol"),
            file_id: b3_core::FileId::new("file"),
            name: "helper".to_string(),
            kind: NodeKind::Function,
            snippet: "fn helper() {}".to_string(),
            start_line: 1,
            end_line: 1,
            visibility: None,
        };

        let symbol_rank = rank_symbol(
            symbol.clone(),
            "helper",
            QueryIntent::SymbolLookup,
            RankingWeights::default(),
            None,
            0,
        );
        let search_rank = rank_symbol(
            symbol,
            "helper",
            QueryIntent::CodeSearch,
            RankingWeights::default(),
            Some(-1.0),
            0,
        );

        assert!(symbol_rank.score != search_rank.score);
        assert!(symbol_rank.ranking_decision.contains("exact_match_score"));
        assert!(search_rank.ranking_decision.contains("bm25_score"));
    }

    #[test]
    fn finds_symbols_from_indexed_rust_data() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());

        let symbols = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("find symbol");

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol.name, "helper");
        assert!(symbols[0].score >= 100);
    }

    #[test]
    fn searches_code_with_fts() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());

        let hits = engine
            .search_code(&fixture.scope, "helper", 10)
            .expect("search");

        assert!(hits.iter().any(|hit| hit.symbol.name == "helper"));
    }

    #[test]
    fn traverses_callers_and_callees_cycle_safely() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let helper = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("helper")
            .remove(0)
            .symbol;
        let entry = engine
            .find_symbol(&fixture.scope, "entry")
            .expect("entry")
            .remove(0)
            .symbol;

        let callers = engine
            .find_callers(&fixture.scope, &helper.id, 4)
            .expect("callers");
        let callees = engine
            .find_callees(&fixture.scope, &entry.id, 4)
            .expect("callees");
        let related = engine
            .related_symbols(&fixture.scope, &helper.id, 2)
            .expect("related");

        assert!(callers.iter().any(|step| step.symbol.name == "entry"));
        assert!(callees.iter().any(|step| step.symbol.name == "helper"));
        assert!(!callers.iter().any(|step| step.symbol.name == "helper"));
        assert!(!callees.iter().any(|step| step.symbol.name == "entry"));
        assert!(related.len() <= DEFAULT_GRAPH_LIMIT);
    }

    #[test]
    fn filters_edges_by_min_confidence() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(
            &fixture.storage,
            QueryEngineConfig {
                min_edge_confidence: 9_500,
                ..QueryEngineConfig::default()
            },
        );
        let helper = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("helper")
            .remove(0)
            .symbol;

        let callers = engine
            .find_callers(&fixture.scope, &helper.id, 2)
            .expect("callers");

        assert!(callers.is_empty());
    }

    #[test]
    fn traversal_prevents_cycles() {
        let fixture = fixture(
            "main",
            r#"
                pub fn a() { b(); }
                pub fn b() { a(); }
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let a = engine
            .find_symbol(&fixture.scope, "a")
            .expect("a")
            .remove(0)
            .symbol;

        let related = engine
            .related_symbols(&fixture.scope, &a.id, 8)
            .expect("related");
        let unique = related
            .iter()
            .map(|step| step.symbol.id.as_str())
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(unique.len(), related.len());
        assert!(related.len() <= 2);
    }

    #[test]
    fn builds_budgeted_deduplicated_context_pack() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let mut symbols = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("find helper");
        symbols.extend(engine.find_symbol(&fixture.scope, "helper").expect("dupe"));

        let pack = engine
            .context_pack_for_symbols(&fixture.scope, &symbols, 80)
            .expect("pack");

        assert_eq!(pack.items.len(), 1);
        assert!(pack.returned_tokens <= 80);
        assert!(pack.items[0].why.contains("exact"));
        assert!(!pack.items[0].source_provenance.is_empty());
        assert!(pack.items[0].snippet.len() <= 600);
        assert_eq!(pack.expansion_handles.len(), 1);
    }

    #[test]
    fn context_pack_reports_truncation_and_stable_handles() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let symbols = engine
            .search_code(&fixture.scope, "helper entry run", 10)
            .expect("search");

        let response = engine
            .context_pack_response_for_symbols(&fixture.scope, &symbols, 8, true)
            .expect("pack response");

        assert!(response.returned_tokens <= response.token_budget);
        assert!(response.truncation_reason.is_some() || !response.skipped_items.is_empty());
        for handle in &response.expansion_handles {
            assert!(handle.starts_with("symbol:"));
        }
        let trace = response.trace.expect("trace");
        assert_eq!(trace.token_budget, 8);
        let serialized = serde_json::to_string(&trace).expect("serialize trace");
        assert!(serialized.contains("get_context_pack"));
    }

    #[test]
    fn context_pack_uses_value_per_token_and_diversity_penalties() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let symbols = engine
            .search_code(&fixture.scope, "helper entry run", 10)
            .expect("search");

        let response = engine
            .context_pack_response_for_symbols(&fixture.scope, &symbols, 400, true)
            .expect("pack response");

        assert!(response
            .items
            .iter()
            .any(|item| item.why.contains("value_per_token")));
        assert!(response
            .items
            .iter()
            .any(|item| item.why.contains("duplicate_file_penalty")));
        assert!(response
            .trace
            .expect("trace")
            .context_items_selected
            .iter()
            .any(|item| item.contains("value_per_token")));
    }

    #[test]
    fn ledger_failure_does_not_fail_context_pack() {
        let fixture = fixture("main", sample_source());
        let repo = FailingLedgerRepo(&fixture.storage);
        let engine = LocalQueryEngine::new(repo, QueryEngineConfig::default());

        let response = engine
            .context_pack_response_for_query(&fixture.scope, "helper", 120, true)
            .expect("context succeeds");

        let trace = response.trace.expect("trace");
        assert!(trace
            .warnings
            .iter()
            .any(|warning| warning.contains("ledger write failed")));
    }

    #[test]
    fn impact_analysis_includes_basic_callers() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let helper = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("helper")
            .remove(0)
            .symbol;

        let impact = engine
            .impact_analysis(&fixture.scope, &helper.id)
            .expect("impact");

        assert!(impact.iter().any(|step| step.symbol.name == "entry"));
        let unique = impact
            .iter()
            .map(|step| step.symbol.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), impact.len());
    }

    #[test]
    fn impact_analysis_scores_low_risk_private_symbol() {
        let fixture = fixture("main", "fn isolated() {}\n");
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let isolated = engine
            .find_symbol(&fixture.scope, "isolated")
            .expect("isolated")
            .remove(0)
            .symbol;

        let response = engine
            .impact_analysis_response(&fixture.scope, &isolated.id, true)
            .expect("impact response");

        assert_eq!(response.risk_level, ImpactRiskLevel::Low);
        assert!(response.risk_score < 25);
        assert!(response.missing_tests);
        assert!(response
            .trace
            .expect("trace")
            .warnings
            .iter()
            .any(|warning| warning.contains("missing tests")));
    }

    #[test]
    fn impact_analysis_scores_high_risk_many_callers() {
        let fixture = fixture(
            "main",
            r#"
                pub fn shared() {}
                fn caller_a() { shared(); }
                fn caller_b() { shared(); }
                fn caller_c() { shared(); }
                fn caller_d() { shared(); }
                fn caller_e() { shared(); }

                #[test]
                fn shared_test() { shared(); }
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let shared = engine
            .find_symbol(&fixture.scope, "shared")
            .expect("shared")
            .remove(0)
            .symbol;

        let response = engine
            .impact_analysis_response(&fixture.scope, &shared.id, true)
            .expect("impact response");

        assert!(matches!(
            response.risk_level,
            ImpactRiskLevel::High | ImpactRiskLevel::Critical
        ));
        assert!(response
            .risk_reasons
            .iter()
            .any(|reason| reason.contains("fan_in")));
        assert!(response.impacted_symbols.len() >= 5);
        assert!(!response.missing_tests);
    }

    #[test]
    fn impact_analysis_boosts_public_api_risk() {
        let fixture = fixture("main", "pub fn exported() {}\nfn private() {}\n");
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let exported = engine
            .find_symbol(&fixture.scope, "exported")
            .expect("exported")
            .remove(0)
            .symbol;
        let private = engine
            .find_symbol(&fixture.scope, "private")
            .expect("private")
            .remove(0)
            .symbol;

        let public_response = engine
            .impact_analysis_response(&fixture.scope, &exported.id, false)
            .expect("public impact");
        let private_response = engine
            .impact_analysis_response(&fixture.scope, &private.id, false)
            .expect("private impact");

        assert!(public_response.risk_score > private_response.risk_score);
        assert!(public_response
            .risk_signals
            .iter()
            .any(|signal| signal.name == "public_api_exposure" && signal.contribution > 0));
    }

    #[test]
    fn impact_analysis_detects_and_ranks_related_tests() {
        let fixture = fixture(
            "main",
            r#"
                fn shared() {}

                #[test]
                fn shared_test() { shared(); }

                #[test]
                fn unrelated_test() {}
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let shared = engine
            .find_symbol(&fixture.scope, "shared")
            .expect("shared")
            .remove(0)
            .symbol;

        let response = engine
            .impact_analysis_response(&fixture.scope, &shared.id, true)
            .expect("impact response");

        assert!(response
            .related_tests
            .iter()
            .any(|test| test.symbol.name == "shared_test" && test.direct));
        assert!(response
            .related_tests
            .windows(2)
            .all(|tests| tests[0].confidence_bps >= tests[1].confidence_bps));
        assert!(response
            .trace
            .expect("trace")
            .ranking_decisions
            .iter()
            .any(|decision| decision.contains("test_match")));
    }

    #[test]
    fn impact_analysis_records_cycles_and_serializes() {
        let fixture = fixture(
            "main",
            r#"
                pub fn a() { b(); }
                pub fn b() { a(); }
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let a = engine
            .find_symbol(&fixture.scope, "a")
            .expect("a")
            .remove(0)
            .symbol;

        let response = engine
            .impact_analysis_response(&fixture.scope, &a.id, true)
            .expect("impact response");
        let serialized = serde_json::to_string(&response).expect("serialize");

        assert!(!response.cycles_involved.is_empty());
        assert!(response
            .risk_signals
            .iter()
            .any(|signal| signal.name == "cycle_presence" && signal.contribution > 0));
        assert!(serialized.contains("risk_score"));
    }

    #[test]
    fn centrality_computes_degree_metrics_and_deterministic_pagerank() {
        let fixture = fixture(
            "main",
            r#"
                fn hub() {}
                fn caller_a() { hub(); }
                fn caller_b() { hub(); }
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());

        let first = engine
            .compute_centrality(&fixture.scope)
            .expect("centrality");
        let second = engine
            .compute_centrality(&fixture.scope)
            .expect("centrality again");
        let hub = first
            .metrics
            .iter()
            .find(|metric| {
                engine
                    .repository
                    .get_symbol(&fixture.scope, &SymbolId::new(metric.symbol_id.clone()))
                    .expect("symbol")
                    .is_some_and(|symbol| symbol.name == "hub")
            })
            .expect("hub metric");

        assert_eq!(first.metrics.len(), second.metrics.len());
        assert_eq!(
            first
                .metrics
                .iter()
                .map(|metric| metric.pagerank_score)
                .collect::<Vec<_>>(),
            second
                .metrics
                .iter()
                .map(|metric| metric.pagerank_score)
                .collect::<Vec<_>>()
        );
        assert_eq!(hub.in_degree, 2);
        assert_eq!(hub.fan_in, 2);
        assert!(hub.pagerank_score > 0.0);
        assert!(fixture
            .storage
            .get_centrality_metric(&fixture.scope, &SymbolId::new(hub.symbol_id.clone()))
            .expect("stored metric")
            .is_some());
    }

    #[test]
    fn centrality_respects_edge_filters_and_max_nodes() {
        let fixture = fixture(
            "main",
            r#"
                fn a() { b(); }
                fn b() { c(); }
                fn c() {}
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let snapshot = engine
            .compute_centrality_with_config(
                &fixture.scope,
                &CentralityConfig {
                    max_nodes: 2,
                    edge_filter: vec![EdgeKind::Imports],
                    ..CentralityConfig::default()
                },
            )
            .expect("centrality");

        assert_eq!(snapshot.metrics.len(), 2);
        assert!(snapshot
            .metrics
            .iter()
            .all(|metric| metric.in_degree == 0 && metric.out_degree == 0));
    }

    #[test]
    fn centrality_preserves_branch_isolation() {
        let dir = tempdir().expect("temp dir");
        let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("open storage");
        let project_id = ProjectId::new("project");

        for (branch, source) in [
            ("main", "fn main_only() {}\nfn caller() { main_only(); }\n"),
            ("feature", "fn feature_only() {}\n"),
        ] {
            let root = dir.path().join(branch);
            fs::create_dir_all(root.join("src")).expect("create src");
            fs::write(root.join("src").join("lib.rs"), source).expect("write branch source");
            let indexer = LocalIndexer::new(
                RustLanguagePack,
                &storage,
                TestEventBus,
                IndexerConfig {
                    branch_id: BranchId::new(branch),
                    ..IndexerConfig::default()
                },
            );
            indexer
                .index(IndexJob {
                    project_id: project_id.clone(),
                    root_path: root.to_string_lossy().to_string(),
                })
                .expect("index branch");
        }

        let engine = LocalQueryEngine::new(&storage, QueryEngineConfig::default());
        let main_scope = QueryScope::new(project_id.clone(), BranchId::new("main"));
        let feature_scope = QueryScope::new(project_id, BranchId::new("feature"));

        let main = engine.compute_centrality(&main_scope).expect("main");
        let feature = engine.compute_centrality(&feature_scope).expect("feature");

        assert_eq!(main.metrics.len(), 2);
        assert_eq!(feature.metrics.len(), 1);
        assert!(engine
            .find_symbol(&feature_scope, "main_only")
            .expect("feature query")
            .is_empty());
    }

    #[test]
    fn impact_analysis_uses_centrality_signal() {
        let fixture = fixture(
            "main",
            r#"
                pub fn hub() {}
                fn caller_a() { hub(); }
                fn caller_b() { hub(); }
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        engine
            .compute_centrality(&fixture.scope)
            .expect("centrality");
        let hub = engine
            .find_symbol(&fixture.scope, "hub")
            .expect("hub")
            .remove(0)
            .symbol;

        let response = engine
            .impact_analysis_response(&fixture.scope, &hub.id, true)
            .expect("impact");

        assert!(response
            .risk_signals
            .iter()
            .any(|signal| signal.name == "centrality" && signal.contribution > 0));
        assert!(response
            .trace
            .expect("trace")
            .ranking_decisions
            .iter()
            .any(|decision| decision.contains("risk_signal centrality")));
    }

    #[test]
    fn context_pack_ranking_uses_centrality() {
        let fixture = fixture(
            "main",
            r#"
                fn hub() {}
                fn leaf() {}
                fn caller_a() { hub(); }
                fn caller_b() { hub(); }
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        engine
            .compute_centrality(&fixture.scope)
            .expect("centrality");

        let search = engine
            .search_code_response(&fixture.scope, "hub leaf", 10, true)
            .expect("search");
        let symbols = engine
            .search_code(&fixture.scope, "hub leaf", 10)
            .expect("ranked");
        let pack = engine
            .context_pack_response_for_symbols(&fixture.scope, &symbols, 400, true)
            .expect("pack");

        assert!(search
            .trace
            .expect("trace")
            .ranking_decisions
            .iter()
            .any(|decision| decision.contains("centrality_score")));
        assert!(pack
            .items
            .iter()
            .any(|item| item.why.contains("centrality boost")));
    }

    #[test]
    fn dependency_path_finds_shortest_path_and_reports_no_path() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let entry = engine
            .find_symbol(&fixture.scope, "entry")
            .expect("entry")
            .remove(0)
            .symbol;
        let helper = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("helper")
            .remove(0)
            .symbol;

        let path = engine
            .dependency_path(
                &fixture.scope,
                &entry.id,
                &helper.id,
                &[EdgeKind::Calls],
                3,
                0,
            )
            .expect("path");
        let no_path = engine
            .dependency_path(
                &fixture.scope,
                &helper.id,
                &entry.id,
                &[EdgeKind::Calls],
                3,
                0,
            )
            .expect("no path");

        assert!(path.found);
        assert_eq!(path.path_length, 1);
        assert!(path.confidence_summary.is_some());
        assert!(!no_path.found);
    }

    #[test]
    fn dependency_path_respects_min_confidence() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());
        let entry = engine
            .find_symbol(&fixture.scope, "entry")
            .expect("entry")
            .remove(0)
            .symbol;
        let helper = engine
            .find_symbol(&fixture.scope, "helper")
            .expect("helper")
            .remove(0)
            .symbol;

        let path = engine
            .dependency_path(
                &fixture.scope,
                &entry.id,
                &helper.id,
                &[EdgeKind::Calls],
                3,
                9_500,
            )
            .expect("path");

        assert!(!path.found);
    }

    #[test]
    fn detects_cycles_with_tarjan_scc() {
        let fixture = fixture(
            "main",
            r#"
                pub fn a() { b(); }
                pub fn b() { a(); }
                pub fn c() {}
            "#,
        );
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());

        let cycles = engine
            .detect_cycles(&fixture.scope, &[EdgeKind::Calls], 100, 0)
            .expect("cycles");

        assert_eq!(cycles.summary_count, 1);
        assert_eq!(cycles.cycles[0].node_ids.len(), 2);
        assert!(cycles.cycles[0].edge_types.contains(&EdgeKind::Calls));
    }

    #[test]
    fn queries_preserve_branch_isolation() {
        let dir = tempdir().expect("temp dir");
        let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("open storage");
        let project_id = ProjectId::new("project");

        for (branch, function) in [("main", "main_only"), ("feature", "feature_only")] {
            let root = dir.path().join(branch);
            fs::create_dir_all(root.join("src")).expect("create src");
            fs::write(
                root.join("src").join("lib.rs"),
                format!("pub fn {function}() {{}}\n"),
            )
            .expect("write branch source");
            let indexer = LocalIndexer::new(
                RustLanguagePack,
                &storage,
                TestEventBus,
                IndexerConfig {
                    branch_id: BranchId::new(branch),
                    ..IndexerConfig::default()
                },
            );
            indexer
                .index(IndexJob {
                    project_id: project_id.clone(),
                    root_path: root.to_string_lossy().to_string(),
                })
                .expect("index branch");
        }

        let engine = LocalQueryEngine::new(&storage, QueryEngineConfig::default());
        let main_scope = QueryScope::new(project_id.clone(), BranchId::new("main"));
        let feature_scope = QueryScope::new(project_id, BranchId::new("feature"));

        assert_eq!(
            engine
                .find_symbol(&main_scope, "feature_only")
                .expect("main query")
                .len(),
            0
        );
        assert_eq!(
            engine
                .find_symbol(&feature_scope, "feature_only")
                .expect("feature query")
                .len(),
            1
        );
        assert!(engine
            .search_code(&main_scope, "feature_only", 10)
            .expect("main fts")
            .is_empty());
        assert_eq!(
            engine
                .detect_cycles(&main_scope, &[EdgeKind::Calls], 100, 0)
                .expect("cycles")
                .scanned_nodes,
            1
        );
    }

    #[test]
    fn trace_includes_adaptive_ranking_decisions() {
        let fixture = fixture("main", sample_source());
        let engine = LocalQueryEngine::new(&fixture.storage, QueryEngineConfig::default());

        let response = engine
            .search_code_response(&fixture.scope, "helper", 10, true)
            .expect("response");
        let trace = response.trace.expect("trace");

        assert!(trace
            .ranking_decisions
            .iter()
            .any(|decision| decision.contains("bm25_score")));
    }
}
