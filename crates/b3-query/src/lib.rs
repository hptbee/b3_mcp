//! Hybrid query and graph retrieval boundary.
//!
//! Query code orchestrates exact lookup, FTS/BM25, graph expansion, ranking,
//! token savings estimates, and context packing through core traits. It does
//! not own storage clients, embedding workers, MCP request handling, or UI.

use std::collections::{HashSet, VecDeque};

use b3_core::{
    ContextItem, ContextPack, ContractError, ContractResult, EdgeKind, GraphDirection,
    GraphNeighbor, QueryEngine, QueryRepository, QueryRequest, QueryResult, QuerySavingsEstimate,
    QueryScope, QuerySymbol, RankingWeights, RetrievalConfig, SymbolId, TokenSavingsRecord,
    TokenSavingsRepository,
};

pub use b3_core::{
    FtsSearchHit as LexicalSearchHit, GraphDirection as TraversalDirection,
    GraphNeighbor as TraversalNeighbor, QueryScope as QueryEngineScope,
};

const DEFAULT_FTS_LIMIT: usize = 20;
const DEFAULT_GRAPH_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryEngineConfig {
    pub retrieval: RetrievalConfig,
    pub ranking: RankingWeights,
    pub min_edge_confidence: u16,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedSymbol {
    pub symbol: QuerySymbol,
    pub score: i64,
    pub why: String,
    pub graph_distance: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraversalStep {
    pub symbol: QuerySymbol,
    pub via: GraphNeighbor,
    pub distance: usize,
}

pub struct LocalQueryEngine<R> {
    repository: R,
    config: QueryEngineConfig,
}

impl<R> LocalQueryEngine<R>
where
    R: QueryRepository + TokenSavingsRepository,
{
    pub fn new(repository: R, config: QueryEngineConfig) -> Self {
        Self { repository, config }
    }

    pub fn find_symbol(&self, scope: &QueryScope, name: &str) -> ContractResult<Vec<RankedSymbol>> {
        let mut symbols = self
            .repository
            .find_symbols(scope, name)?
            .into_iter()
            .map(|symbol| RankedSymbol {
                score: i64::from(self.config.ranking.exact_symbol),
                why: "exact symbol match".to_string(),
                graph_distance: 0,
                symbol,
            })
            .collect::<Vec<_>>();

        symbols.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        Ok(symbols)
    }

    pub fn search_code(
        &self,
        scope: &QueryScope,
        query: &str,
        limit: usize,
    ) -> ContractResult<Vec<RankedSymbol>> {
        let mut ranked = Vec::new();
        let mut seen = HashSet::new();

        for hit in self.repository.fts_search(scope, query, limit)? {
            if let Some(symbol_id) = hit.symbol_id {
                if seen.insert(symbol_id.as_str().to_string()) {
                    if let Some(symbol) = self.repository.get_symbol(scope, &symbol_id)? {
                        ranked.push(RankedSymbol {
                            symbol,
                            score: lexical_score(hit.score, self.config.ranking.lexical_bm25),
                            why: "FTS/BM25 lexical match".to_string(),
                            graph_distance: 0,
                        });
                    }
                }
            }
        }

        ranked.sort_by(|left, right| right.score.cmp(&left.score));
        Ok(ranked)
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

    pub fn context_pack_for_symbols(
        &self,
        scope: &QueryScope,
        symbols: &[RankedSymbol],
        token_budget: usize,
    ) -> ContractResult<ContextPack> {
        let mut items = Vec::new();
        let mut handles = Vec::new();
        let mut seen = HashSet::new();
        let mut returned_tokens = 0;

        for ranked in symbols {
            let key = ranked.symbol.id.as_str().to_string();
            if !seen.insert(key) {
                continue;
            }

            let Some(file) = self.repository.get_file(scope, &ranked.symbol.file_id)? else {
                continue;
            };
            let snippet = compact_snippet(&ranked.symbol.snippet);
            let estimated_tokens = estimate_tokens(&snippet) + estimate_tokens(&ranked.symbol.name);
            if returned_tokens + estimated_tokens > token_budget && !items.is_empty() {
                break;
            }

            let handle = format!("symbol:{}", ranked.symbol.id.as_str());
            handles.push(handle.clone());
            returned_tokens += estimated_tokens;
            items.push(ContextItem {
                file_id: file.id,
                symbol_id: Some(ranked.symbol.id.clone()),
                title: format!("{} ({})", ranked.symbol.name, file.path),
                snippet,
                why: ranked.why.clone(),
                estimated_tokens,
                expansion_handle: handle,
            });
        }

        self.record_savings(QuerySavingsEstimate {
            tool_call_id: None,
            returned_tokens,
            avoided_file_reads: items.len(),
            avoided_search_calls: 1,
        })?;

        Ok(ContextPack {
            items,
            returned_tokens,
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

        steps.sort_by(|left, right| {
            left.distance
                .cmp(&right.distance)
                .then_with(|| {
                    right
                        .via
                        .confidence
                        .basis_points()
                        .cmp(&left.via.confidence.basis_points())
                })
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        Ok(steps)
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

impl<R> QueryEngine for LocalQueryEngine<R>
where
    R: QueryRepository + TokenSavingsRepository,
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
            } else {
                neighbor.from_symbol.clone()
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{BranchId, ContractResult, DomainEvent, EventBus, IndexJob, Indexer, ProjectId};
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
        assert!(related.len() <= DEFAULT_GRAPH_LIMIT);
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
        assert_eq!(pack.expansion_handles.len(), 1);
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
    }
}
