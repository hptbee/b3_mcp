//! Diff-aware impact analysis over existing indexed evidence.
//!
//! This module consumes already-bounded Git status/diff DTOs and indexed B3
//! metadata. It never executes Git commands, reads patches, mutates Git state,
//! or changes extraction/ranking behavior.

use std::collections::{BTreeSet, HashSet};

use b3_core::{
    ContractResult, GitChangedFile, GitChangedFileStatus, GitCompareResult, GitDiffSummary,
    GitIndexFreshness, GitIndexFreshnessStatus, NodeKind, ProjectId, QueryRepository, QueryScope,
};
use b3_storage::{
    SqliteStorage, StoredComponent, StoredDataAccess, StoredInfrastructure, StoredMessaging,
    StoredRealtime, StoredRoute, StoredWpf,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactProfile {
    Minimal,
    Balanced,
    Deep,
}

impl Default for ImpactProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffImpactRequest {
    pub project_id: String,
    pub branch_id: String,
    pub project_path: Option<String>,
    pub database_path: Option<String>,
    pub include_untracked: bool,
    pub include_line_counts: bool,
    pub profile: ImpactProfile,
    pub max_changed_files: usize,
    pub max_impacted_items: usize,
    pub include_context_pack: bool,
    pub include_architecture: bool,
}

impl Default for GitDiffImpactRequest {
    fn default() -> Self {
        Self {
            project_id: "default".to_string(),
            branch_id: "main".to_string(),
            project_path: None,
            database_path: None,
            include_untracked: true,
            include_line_counts: true,
            profile: ImpactProfile::Balanced,
            max_changed_files: 100,
            max_impacted_items: 100,
            include_context_pack: true,
            include_architecture: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffImpactResult {
    pub freshness: Option<GitIndexFreshness>,
    pub diff_summary: GitDiffSummary,
    pub impacted_files: Vec<GitImpactedFile>,
    pub impacted_symbols: Vec<GitImpactedSymbol>,
    pub impacted_routes: Vec<GitImpactedRoute>,
    pub impacted_components: Vec<GitImpactedComponent>,
    pub impacted_data_access: Vec<GitImpactedDataAccess>,
    pub impacted_realtime: Vec<GitImpactedRealtime>,
    pub impacted_messaging: Vec<GitImpactedMessaging>,
    pub impacted_infrastructure: Vec<GitImpactedInfrastructure>,
    pub impacted_wpf: Vec<GitImpactedWpf>,
    pub impacted_sql: Vec<GitImpactedSql>,
    pub impacted_architecture: Vec<GitImpactedArchitectureItem>,
    pub recommended_context_seeds: Vec<GitContextSeed>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitImpactSeverity {
    Direct,
    Related,
    Transitive,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitImpactReason {
    ChangedFile,
    DeletedFile,
    RenamedFile,
    SymbolInChangedFile,
    RouteInChangedFile,
    ComponentInChangedFile,
    DataAccessInChangedFile,
    RealtimeInChangedFile,
    MessageProducerConsumerInChangedFile,
    InfrastructureInChangedFile,
    WpfBindingInChangedFile,
    SqlObjectInChangedFile,
    ReferenceToChangedSymbol,
    ArchitectureMatch,
    StaleIndexWarning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitImpactedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: GitChangedFileStatus,
    pub severity: GitImpactSeverity,
    pub reasons: Vec<GitImpactReason>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
    pub lines_added: Option<u64>,
    pub lines_deleted: Option<u64>,
    pub indexed: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitImpactedEvidence {
    pub id: String,
    pub symbol_id: Option<String>,
    pub name: String,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub severity: GitImpactSeverity,
    pub reasons: Vec<GitImpactReason>,
    pub summary: String,
    pub confidence: Option<u16>,
}

pub type GitImpactedSymbol = GitImpactedEvidence;
pub type GitImpactedRoute = GitImpactedEvidence;
pub type GitImpactedComponent = GitImpactedEvidence;
pub type GitImpactedDataAccess = GitImpactedEvidence;
pub type GitImpactedRealtime = GitImpactedEvidence;
pub type GitImpactedMessaging = GitImpactedEvidence;
pub type GitImpactedInfrastructure = GitImpactedEvidence;
pub type GitImpactedWpf = GitImpactedEvidence;
pub type GitImpactedSql = GitImpactedEvidence;
pub type GitImpactedArchitectureItem = GitImpactedEvidence;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitContextSeed {
    pub seed_type: String,
    pub id: String,
    pub file_path: String,
    pub reason: GitImpactReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedSymbolEvidence {
    pub id: String,
    pub file_path: String,
    pub name: String,
    pub kind: NodeKind,
    pub snippet: String,
    pub line_start: usize,
    pub line_end: usize,
    pub visibility: Option<String>,
}

pub trait GitImpactRepository {
    fn symbols_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<IndexedSymbolEvidence>>;

    fn routes_for_impact(
        &self,
        _scope: &QueryScope,
        _limit: usize,
    ) -> ContractResult<Vec<StoredRoute>> {
        Ok(Vec::new())
    }

    fn components_for_impact(
        &self,
        _scope: &QueryScope,
        _limit: usize,
    ) -> ContractResult<Vec<StoredComponent>> {
        Ok(Vec::new())
    }

    fn data_access_for_impact(
        &self,
        _scope: &QueryScope,
        _limit: usize,
    ) -> ContractResult<Vec<StoredDataAccess>> {
        Ok(Vec::new())
    }

    fn realtime_for_impact(
        &self,
        _scope: &QueryScope,
        _limit: usize,
    ) -> ContractResult<Vec<StoredRealtime>> {
        Ok(Vec::new())
    }

    fn messaging_for_impact(
        &self,
        _scope: &QueryScope,
        _limit: usize,
    ) -> ContractResult<Vec<StoredMessaging>> {
        Ok(Vec::new())
    }

    fn infrastructure_for_impact(
        &self,
        _scope: &QueryScope,
        _limit: usize,
    ) -> ContractResult<Vec<StoredInfrastructure>> {
        Ok(Vec::new())
    }

    fn wpf_for_impact(&self, _scope: &QueryScope, _limit: usize) -> ContractResult<Vec<StoredWpf>> {
        Ok(Vec::new())
    }
}

impl GitImpactRepository for SqliteStorage {
    fn symbols_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<IndexedSymbolEvidence>> {
        symbols_from_query_repository(self, scope, limit)
    }

    fn routes_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredRoute>> {
        self.routes(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }

    fn components_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredComponent>> {
        self.components(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }

    fn data_access_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredDataAccess>> {
        self.data_access(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            None,
            limit,
        )
    }

    fn realtime_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredRealtime>> {
        self.realtime(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            None,
            limit,
        )
    }

    fn messaging_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredMessaging>> {
        self.messaging(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            None,
            None,
            limit,
        )
    }

    fn infrastructure_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredInfrastructure>> {
        self.infrastructure(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }

    fn wpf_for_impact(&self, scope: &QueryScope, limit: usize) -> ContractResult<Vec<StoredWpf>> {
        self.wpf(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }
}

impl GitImpactRepository for &SqliteStorage {
    fn symbols_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<IndexedSymbolEvidence>> {
        symbols_from_query_repository(*self, scope, limit)
    }

    fn routes_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredRoute>> {
        (*self).routes(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }

    fn components_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredComponent>> {
        (*self).components(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }

    fn data_access_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredDataAccess>> {
        (*self).data_access(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            None,
            limit,
        )
    }

    fn realtime_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredRealtime>> {
        (*self).realtime(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            None,
            limit,
        )
    }

    fn messaging_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredMessaging>> {
        (*self).messaging(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            None,
            None,
            limit,
        )
    }

    fn infrastructure_for_impact(
        &self,
        scope: &QueryScope,
        limit: usize,
    ) -> ContractResult<Vec<StoredInfrastructure>> {
        (*self).infrastructure(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }

    fn wpf_for_impact(&self, scope: &QueryScope, limit: usize) -> ContractResult<Vec<StoredWpf>> {
        (*self).wpf(
            scope.project_id.as_str(),
            scope.branch_id.as_str(),
            None,
            None,
            None,
            limit,
        )
    }
}

pub fn analyze_git_diff_impact<R: GitImpactRepository>(
    repository: &R,
    request: &GitDiffImpactRequest,
    freshness: Option<GitIndexFreshness>,
    diff_summary: GitDiffSummary,
) -> ContractResult<GitDiffImpactResult> {
    let scope = QueryScope::new(
        ProjectId::new(request.project_id.clone()),
        b3_core::BranchId::new(request.branch_id.clone()),
    );
    let max_changed = request.max_changed_files.max(1);
    let max_items = request.max_impacted_items.max(1);
    let changed = diff_summary
        .changed_files
        .iter()
        .filter(|file| request.include_untracked || !file.untracked)
        .take(max_changed)
        .cloned()
        .collect::<Vec<_>>();
    let changed_paths = changed_path_set(&changed);
    let truncated = diff_summary.truncated
        || diff_summary.changed_files.len() > changed.len()
        || diff_summary.total_changed_count > max_changed;

    let symbols = repository.symbols_for_impact(&scope, max_items.saturating_mul(8))?;
    let impacted_symbol_ids = symbols
        .iter()
        .filter(|symbol| path_matches(&symbol.file_path, &changed_paths))
        .map(|symbol| symbol.id.clone())
        .collect::<BTreeSet<_>>();

    let impacted_files = changed
        .iter()
        .map(|file| impacted_file(file, &symbols, &changed_paths))
        .collect::<Vec<_>>();
    let impacted_symbols = take_bounded(
        symbols
            .iter()
            .filter(|symbol| path_matches(&symbol.file_path, &changed_paths))
            .map(symbol_evidence),
        max_items,
    );
    let impacted_routes = take_bounded(
        repository
            .routes_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(route_evidence),
        max_items,
    );
    let impacted_components = take_bounded(
        repository
            .components_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(component_evidence),
        max_items,
    );
    let impacted_data_access = take_bounded(
        repository
            .data_access_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(data_access_evidence),
        max_items,
    );
    let impacted_realtime = take_bounded(
        repository
            .realtime_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(realtime_evidence),
        max_items,
    );
    let impacted_messaging = take_bounded(
        repository
            .messaging_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(messaging_evidence),
        max_items,
    );
    let impacted_infrastructure = take_bounded(
        repository
            .infrastructure_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(infrastructure_evidence),
        max_items,
    );
    let impacted_wpf = take_bounded(
        repository
            .wpf_for_impact(&scope, max_items.saturating_mul(4))?
            .into_iter()
            .filter(|record| path_matches(&record.file_path, &changed_paths))
            .map(wpf_evidence),
        max_items,
    );
    let impacted_sql = take_bounded(
        symbols
            .iter()
            .filter(|symbol| path_matches(&symbol.file_path, &changed_paths))
            .filter(|symbol| is_sql_or_ksql_symbol(symbol))
            .map(sql_evidence),
        max_items,
    );
    let impacted_architecture = if request.include_architecture {
        architecture_items(
            &impacted_routes,
            &impacted_messaging,
            &impacted_infrastructure,
            max_items,
        )
    } else {
        Vec::new()
    };

    let mut warnings = diff_summary.warnings.clone();
    if truncated {
        warnings.push("diff impact is truncated by configured bounds".to_string());
    }
    for file in &impacted_files {
        warnings.extend(file.warnings.iter().cloned());
    }
    warnings.extend(freshness_warnings(freshness.as_ref()));

    let recommended_context_seeds = if request.include_context_pack {
        context_seeds(
            &impacted_files,
            &impacted_symbols,
            &impacted_routes,
            &impacted_messaging,
            max_items,
        )
    } else {
        Vec::new()
    };

    if !impacted_symbol_ids.is_empty() {
        warnings.push(format!(
            "reference expansion deferred; {} changed symbols are available as context seeds",
            impacted_symbol_ids.len()
        ));
    }

    Ok(GitDiffImpactResult {
        freshness,
        diff_summary,
        impacted_files,
        impacted_symbols,
        impacted_routes,
        impacted_components,
        impacted_data_access,
        impacted_realtime,
        impacted_messaging,
        impacted_infrastructure,
        impacted_wpf,
        impacted_sql,
        impacted_architecture,
        recommended_context_seeds,
        warnings: dedupe_strings(warnings),
        truncated,
    })
}

pub fn analyze_git_compare_impact<R: GitImpactRepository>(
    repository: &R,
    request: &GitDiffImpactRequest,
    freshness: Option<GitIndexFreshness>,
    compare_result: &GitCompareResult,
) -> ContractResult<Option<GitDiffImpactResult>> {
    let Some(diff_summary) = compare_result.diff_summary.clone() else {
        return Ok(None);
    };
    let mut result = analyze_git_diff_impact(repository, request, freshness, diff_summary)?;
    result
        .warnings
        .extend(compare_result.warnings.iter().cloned());
    result.warnings = dedupe_strings(result.warnings);
    if compare_result.truncated {
        result.truncated = true;
    }
    Ok(Some(result))
}

fn symbols_from_query_repository<R: QueryRepository>(
    repository: &R,
    scope: &QueryScope,
    limit: usize,
) -> ContractResult<Vec<IndexedSymbolEvidence>> {
    let mut output = Vec::new();
    for symbol in repository.list_symbols(scope, limit)? {
        let Some(file) = repository.get_file(scope, &symbol.file_id)? else {
            continue;
        };
        output.push(IndexedSymbolEvidence {
            id: symbol.id.as_str().to_string(),
            file_path: file.path,
            name: symbol.name,
            kind: symbol.kind,
            snippet: symbol.snippet,
            line_start: symbol.start_line,
            line_end: symbol.end_line,
            visibility: symbol.visibility,
        });
    }
    Ok(output)
}

fn changed_path_set(files: &[GitChangedFile]) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for file in files {
        paths.insert(normalize_path(&file.path));
        if let Some(old_path) = &file.old_path {
            paths.insert(normalize_path(old_path));
        }
    }
    paths
}

fn path_matches(path: &str, changed_paths: &BTreeSet<String>) -> bool {
    changed_paths.contains(&normalize_path(path))
}

fn impacted_file(
    file: &GitChangedFile,
    symbols: &[IndexedSymbolEvidence],
    changed_paths: &BTreeSet<String>,
) -> GitImpactedFile {
    let indexed = symbols
        .iter()
        .any(|symbol| path_matches(&symbol.file_path, changed_paths));
    let mut warnings = file.warnings.clone();
    if !indexed && !file.untracked {
        warnings.push(format!(
            "changed path has no indexed evidence: {}",
            file.path
        ));
    }
    GitImpactedFile {
        path: file.path.clone(),
        old_path: file.old_path.clone(),
        status: file.status,
        severity: if indexed {
            GitImpactSeverity::Direct
        } else {
            GitImpactSeverity::Unknown
        },
        reasons: file_reasons(file),
        staged: file.staged,
        unstaged: file.unstaged,
        untracked: file.untracked,
        conflicted: file.conflicted,
        lines_added: file.lines_added,
        lines_deleted: file.lines_deleted,
        indexed,
        warnings,
    }
}

fn file_reasons(file: &GitChangedFile) -> Vec<GitImpactReason> {
    match file.status {
        GitChangedFileStatus::Deleted => vec![GitImpactReason::DeletedFile],
        GitChangedFileStatus::Renamed => vec![GitImpactReason::RenamedFile],
        _ => vec![GitImpactReason::ChangedFile],
    }
}

fn symbol_evidence(symbol: &IndexedSymbolEvidence) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: symbol.id.clone(),
        symbol_id: Some(symbol.id.clone()),
        name: symbol.name.clone(),
        file_path: symbol.file_path.clone(),
        line_start: symbol.line_start,
        line_end: symbol.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::SymbolInChangedFile],
        summary: format!("{:?} {}", symbol.kind, symbol.name),
        confidence: None,
    }
}

fn route_evidence(record: StoredRoute) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: format!("{} {}", record.method, record.path),
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::RouteInChangedFile],
        summary: format!("{} route via {}", record.framework, record.source_kind),
        confidence: Some(record.confidence),
    }
}

fn component_evidence(record: StoredComponent) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: record.name,
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::ComponentInChangedFile],
        summary: format!("{} component via {}", record.framework, record.source_kind),
        confidence: Some(record.confidence),
    }
}

fn data_access_evidence(record: StoredDataAccess) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: record
            .entity_name
            .clone()
            .or(record.method_name.clone())
            .unwrap_or_else(|| record.kind.clone()),
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::DataAccessInChangedFile],
        summary: format!("{} {} data access", record.technology, record.kind),
        confidence: Some(record.confidence),
    }
}

fn realtime_evidence(record: StoredRealtime) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: record
            .event_name
            .clone()
            .or(record.channel_name.clone())
            .or(record.hub_name.clone())
            .unwrap_or_else(|| record.kind.clone()),
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::RealtimeInChangedFile],
        summary: format!("{} {} realtime", record.technology, record.direction),
        confidence: Some(record.confidence),
    }
}

fn messaging_evidence(record: StoredMessaging) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: record
            .topic
            .clone()
            .or(record.queue.clone())
            .or(record.routing_key.clone())
            .unwrap_or_else(|| record.kind.clone()),
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::MessageProducerConsumerInChangedFile],
        summary: format!("{} {} messaging", record.technology, record.direction),
        confidence: Some(record.confidence),
    }
}

fn infrastructure_evidence(record: StoredInfrastructure) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: record.name.clone().unwrap_or_else(|| record.kind.clone()),
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::InfrastructureInChangedFile],
        summary: format!("{} infrastructure", record.technology),
        confidence: Some(record.confidence),
    }
}

fn wpf_evidence(record: StoredWpf) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: record.id,
        symbol_id: Some(record.symbol_id),
        name: record.name.clone().unwrap_or_else(|| record.kind.clone()),
        file_path: record.file_path,
        line_start: record.line_start,
        line_end: record.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::WpfBindingInChangedFile],
        summary: format!("{} WPF", record.kind),
        confidence: Some(record.confidence),
    }
}

fn sql_evidence(symbol: &IndexedSymbolEvidence) -> GitImpactedEvidence {
    GitImpactedEvidence {
        id: symbol.id.clone(),
        symbol_id: Some(symbol.id.clone()),
        name: symbol.name.clone(),
        file_path: symbol.file_path.clone(),
        line_start: symbol.line_start,
        line_end: symbol.line_end,
        severity: GitImpactSeverity::Direct,
        reasons: vec![GitImpactReason::SqlObjectInChangedFile],
        summary: symbol
            .visibility
            .clone()
            .unwrap_or_else(|| "SQL/ksqlDB metadata".to_string()),
        confidence: None,
    }
}

fn architecture_items(
    routes: &[GitImpactedRoute],
    messaging: &[GitImpactedMessaging],
    infrastructure: &[GitImpactedInfrastructure],
    max_items: usize,
) -> Vec<GitImpactedArchitectureItem> {
    routes
        .iter()
        .chain(messaging.iter())
        .chain(infrastructure.iter())
        .take(max_items)
        .map(|item| {
            let mut item = item.clone();
            item.severity = GitImpactSeverity::Related;
            item.reasons = vec![GitImpactReason::ArchitectureMatch];
            item
        })
        .collect()
}

fn context_seeds(
    files: &[GitImpactedFile],
    symbols: &[GitImpactedSymbol],
    routes: &[GitImpactedRoute],
    messaging: &[GitImpactedMessaging],
    max_items: usize,
) -> Vec<GitContextSeed> {
    let mut seeds = Vec::new();
    for file in files.iter().take(max_items) {
        seeds.push(GitContextSeed {
            seed_type: "file".to_string(),
            id: file.path.clone(),
            file_path: file.path.clone(),
            reason: GitImpactReason::ChangedFile,
        });
    }
    for item in symbols.iter().take(max_items) {
        seeds.push(seed_from_evidence(
            "symbol",
            item,
            GitImpactReason::SymbolInChangedFile,
        ));
    }
    for item in routes.iter().take(max_items) {
        seeds.push(seed_from_evidence(
            "route",
            item,
            GitImpactReason::RouteInChangedFile,
        ));
    }
    for item in messaging.iter().take(max_items) {
        seeds.push(seed_from_evidence(
            "messaging",
            item,
            GitImpactReason::MessageProducerConsumerInChangedFile,
        ));
    }
    dedupe_seeds(seeds).into_iter().take(max_items).collect()
}

fn seed_from_evidence(
    seed_type: &str,
    item: &GitImpactedEvidence,
    reason: GitImpactReason,
) -> GitContextSeed {
    GitContextSeed {
        seed_type: seed_type.to_string(),
        id: item.id.clone(),
        file_path: item.file_path.clone(),
        reason,
    }
}

fn freshness_warnings(freshness: Option<&GitIndexFreshness>) -> Vec<String> {
    let Some(freshness) = freshness else {
        return vec!["freshness unavailable; impact is conservative".to_string()];
    };
    let mut warnings = freshness.warnings.clone();
    match freshness.status {
        GitIndexFreshnessStatus::Fresh => {}
        GitIndexFreshnessStatus::Dirty => {
            warnings.push("working tree is dirty; impact uses current diff summary".to_string())
        }
        GitIndexFreshnessStatus::Stale => warnings.push(format!(
            "index is stale; manual reindex recommended: {}",
            freshness.recommendation
        )),
        GitIndexFreshnessStatus::Unsafe => warnings.push(format!(
            "index state is unsafe; manual action required: {}",
            freshness.recommendation
        )),
        GitIndexFreshnessStatus::Unknown => {
            warnings.push("index freshness is unknown; impact is conservative".to_string())
        }
    }
    if freshness.manual_action_required {
        warnings.push("manual action required before trusting full impact".to_string());
    }
    warnings
}

fn is_sql_or_ksql_symbol(symbol: &IndexedSymbolEvidence) -> bool {
    symbol.visibility.as_ref().is_some_and(|metadata| {
        metadata.contains("sql.") || metadata.contains("ksqldb.") || metadata.contains("ksql")
    })
}

fn take_bounded<T>(items: impl Iterator<Item = T>, max_items: usize) -> Vec<T> {
    items.take(max_items).collect()
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn dedupe_seeds(values: Vec<GitContextSeed>) -> Vec<GitContextSeed> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|seed| seen.insert((seed.seed_type.clone(), seed.id.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{
        AutoIndexDecision, AutoIndexMode, GitIndexFreshnessStatus, GitStaleReason,
        GitWorkingTreeStatus, SymbolId,
    };

    #[derive(Default)]
    struct FixtureImpactRepository {
        symbols: Vec<IndexedSymbolEvidence>,
        routes: Vec<StoredRoute>,
        components: Vec<StoredComponent>,
        data_access: Vec<StoredDataAccess>,
        messaging: Vec<StoredMessaging>,
    }

    impl GitImpactRepository for FixtureImpactRepository {
        fn symbols_for_impact(
            &self,
            _scope: &QueryScope,
            _limit: usize,
        ) -> ContractResult<Vec<IndexedSymbolEvidence>> {
            Ok(self.symbols.clone())
        }

        fn routes_for_impact(
            &self,
            _scope: &QueryScope,
            _limit: usize,
        ) -> ContractResult<Vec<StoredRoute>> {
            Ok(self.routes.clone())
        }

        fn components_for_impact(
            &self,
            _scope: &QueryScope,
            _limit: usize,
        ) -> ContractResult<Vec<StoredComponent>> {
            Ok(self.components.clone())
        }

        fn data_access_for_impact(
            &self,
            _scope: &QueryScope,
            _limit: usize,
        ) -> ContractResult<Vec<StoredDataAccess>> {
            Ok(self.data_access.clone())
        }

        fn messaging_for_impact(
            &self,
            _scope: &QueryScope,
            _limit: usize,
        ) -> ContractResult<Vec<StoredMessaging>> {
            Ok(self.messaging.clone())
        }
    }

    #[test]
    fn changed_file_maps_to_indexed_symbols_routes_and_context_seeds() {
        let repo = FixtureImpactRepository {
            symbols: vec![symbol("sym-user", "src/users.rs", "list_users", None)],
            routes: vec![route("route-users", "src/users.rs", "GET", "/users")],
            ..FixtureImpactRepository::default()
        };
        let result = analyze_git_diff_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Fresh)),
            diff(vec![changed(
                "src/users.rs",
                GitChangedFileStatus::Modified,
            )]),
        )
        .expect("impact");

        assert_eq!(result.impacted_files[0].severity, GitImpactSeverity::Direct);
        assert_eq!(result.impacted_symbols.len(), 1);
        assert_eq!(result.impacted_routes.len(), 1);
        assert!(result
            .recommended_context_seeds
            .iter()
            .any(|seed| seed.seed_type == "symbol"));
    }

    #[test]
    fn windows_separator_changed_path_maps_to_indexed_file() {
        let repo = FixtureImpactRepository {
            symbols: vec![symbol("sym-user", "src/users.rs", "list_users", None)],
            ..FixtureImpactRepository::default()
        };
        let result = analyze_git_diff_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Fresh)),
            diff(vec![changed(
                "src\\users.rs",
                GitChangedFileStatus::Modified,
            )]),
        )
        .expect("impact");

        assert!(result.impacted_files[0].indexed);
        assert_eq!(result.impacted_symbols.len(), 1);
    }

    #[test]
    fn renamed_file_maps_old_path_to_indexed_evidence() {
        let repo = FixtureImpactRepository {
            symbols: vec![symbol("sym-user", "src/old.rs", "list_users", None)],
            ..FixtureImpactRepository::default()
        };
        let mut file = changed("src/new.rs", GitChangedFileStatus::Renamed);
        file.old_path = Some("src/old.rs".to_string());
        let result = analyze_git_diff_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Fresh)),
            diff(vec![file]),
        )
        .expect("impact");

        assert!(result.impacted_files[0]
            .reasons
            .contains(&GitImpactReason::RenamedFile));
        assert_eq!(result.impacted_symbols.len(), 1);
    }

    #[test]
    fn untracked_file_is_unknown_when_not_indexed() {
        let repo = FixtureImpactRepository::default();
        let mut file = changed("src/new.rs", GitChangedFileStatus::Untracked);
        file.untracked = true;
        let result = analyze_git_diff_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Dirty)),
            diff(vec![file]),
        )
        .expect("impact");

        assert_eq!(
            result.impacted_files[0].severity,
            GitImpactSeverity::Unknown
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("working tree is dirty")));
    }

    #[test]
    fn direct_surfaces_include_component_messaging_data_access_and_sql() {
        let repo = FixtureImpactRepository {
            symbols: vec![symbol(
                "sql-orders",
                "db/orders.sql",
                "orders",
                Some("sql.file=true;data_access.technology=sql"),
            )],
            components: vec![component(
                "component-orders",
                "db/orders.sql",
                "OrdersPanel",
            )],
            messaging: vec![messaging(
                "message-orders",
                "db/orders.sql",
                "orders.created",
            )],
            data_access: vec![data_access("data-orders", "db/orders.sql", "orders")],
            ..FixtureImpactRepository::default()
        };
        let result = analyze_git_diff_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Fresh)),
            diff(vec![changed(
                "db/orders.sql",
                GitChangedFileStatus::Modified,
            )]),
        )
        .expect("impact");

        assert_eq!(result.impacted_components.len(), 1);
        assert_eq!(result.impacted_messaging.len(), 1);
        assert_eq!(result.impacted_data_access.len(), 1);
        assert_eq!(result.impacted_sql.len(), 1);
    }

    #[test]
    fn stale_and_unsafe_freshness_warnings_are_included() {
        let repo = FixtureImpactRepository::default();
        let result = analyze_git_diff_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Stale)),
            diff(vec![changed(
                "src/users.rs",
                GitChangedFileStatus::Modified,
            )]),
        )
        .expect("impact");

        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("index is stale")));
    }

    #[test]
    fn bounds_and_truncated_diff_propagate_warning() {
        let repo = FixtureImpactRepository {
            symbols: vec![
                symbol("one", "a.rs", "one", None),
                symbol("two", "b.rs", "two", None),
            ],
            ..FixtureImpactRepository::default()
        };
        let mut request = request();
        request.max_changed_files = 1;
        request.max_impacted_items = 1;
        let mut summary = diff(vec![
            changed("a.rs", GitChangedFileStatus::Modified),
            changed("b.rs", GitChangedFileStatus::Modified),
        ]);
        summary.truncated = true;
        let result = analyze_git_diff_impact(&repo, &request, None, summary).expect("impact");

        assert!(result.truncated);
        assert_eq!(result.impacted_files.len(), 1);
        assert_eq!(result.impacted_symbols.len(), 1);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("truncated")));
    }

    #[test]
    fn architecture_items_are_direct_evidence_only_when_requested() {
        let repo = FixtureImpactRepository {
            routes: vec![route("route-users", "src/users.rs", "GET", "/users")],
            messaging: vec![messaging("message-users", "src/users.rs", "users.changed")],
            ..FixtureImpactRepository::default()
        };
        let mut request = request();
        request.include_architecture = true;
        let result = analyze_git_diff_impact(
            &repo,
            &request,
            Some(freshness(GitIndexFreshnessStatus::Fresh)),
            diff(vec![changed(
                "src/users.rs",
                GitChangedFileStatus::Modified,
            )]),
        )
        .expect("impact");

        assert_eq!(result.impacted_architecture.len(), 2);
        assert!(result
            .impacted_architecture
            .iter()
            .all(|item| item.reasons == vec![GitImpactReason::ArchitectureMatch]));
    }

    #[test]
    fn compare_result_can_feed_diff_impact_path_mapping() {
        let repo = FixtureImpactRepository {
            symbols: vec![symbol("sym-user", "src/users.rs", "list_users", None)],
            ..FixtureImpactRepository::default()
        };
        let compare = b3_core::GitCompareResult {
            is_git_repo: true,
            repo_root: Some(".".to_string()),
            base_ref: Some("main".to_string()),
            base_commit: Some("base".to_string()),
            head_ref: Some("HEAD".to_string()),
            head_commit: Some("head".to_string()),
            merge_base: Some("base".to_string()),
            diff_mode: b3_core::GitCompareDiffMode::MergeBaseTripleDot,
            diff_summary: Some(diff(vec![changed(
                "src/users.rs",
                GitChangedFileStatus::Modified,
            )])),
            warnings: vec!["compare warning".to_string()],
            truncated: false,
        };
        let result = analyze_git_compare_impact(
            &repo,
            &request(),
            Some(freshness(GitIndexFreshnessStatus::Fresh)),
            &compare,
        )
        .expect("impact")
        .expect("result");

        assert_eq!(result.impacted_symbols.len(), 1);
        assert!(result.warnings.contains(&"compare warning".to_string()));
    }

    fn request() -> GitDiffImpactRequest {
        GitDiffImpactRequest {
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            max_changed_files: 10,
            max_impacted_items: 10,
            ..GitDiffImpactRequest::default()
        }
    }

    fn diff(files: Vec<GitChangedFile>) -> GitDiffSummary {
        GitDiffSummary {
            is_git_repo: true,
            repo_root: Some(".".to_string()),
            base_ref: None,
            head_ref: Some("HEAD".to_string()),
            staged_count: files.iter().filter(|file| file.staged).count(),
            unstaged_count: files.iter().filter(|file| file.unstaged).count(),
            untracked_count: files.iter().filter(|file| file.untracked).count(),
            conflicted_count: files.iter().filter(|file| file.conflicted).count(),
            added_count: files
                .iter()
                .filter(|file| file.status == GitChangedFileStatus::Added)
                .count(),
            modified_count: files
                .iter()
                .filter(|file| file.status == GitChangedFileStatus::Modified)
                .count(),
            deleted_count: files
                .iter()
                .filter(|file| file.status == GitChangedFileStatus::Deleted)
                .count(),
            renamed_count: files
                .iter()
                .filter(|file| file.status == GitChangedFileStatus::Renamed)
                .count(),
            copied_count: files
                .iter()
                .filter(|file| file.status == GitChangedFileStatus::Copied)
                .count(),
            total_changed_count: files.len(),
            total_lines_added: Some(0),
            total_lines_deleted: Some(0),
            changed_files: files,
            truncated: false,
            warnings: Vec::new(),
        }
    }

    fn changed(path: &str, status: GitChangedFileStatus) -> GitChangedFile {
        GitChangedFile {
            path: path.to_string(),
            old_path: None,
            status,
            staged: false,
            unstaged: true,
            untracked: false,
            conflicted: false,
            lines_added: Some(2),
            lines_deleted: Some(1),
            language: None,
            is_indexed: None,
            warnings: Vec::new(),
        }
    }

    fn symbol(
        id: &str,
        file_path: &str,
        name: &str,
        visibility: Option<&str>,
    ) -> IndexedSymbolEvidence {
        IndexedSymbolEvidence {
            id: SymbolId::new(id).as_str().to_string(),
            file_path: file_path.to_string(),
            name: name.to_string(),
            kind: NodeKind::Function,
            snippet: name.to_string(),
            line_start: 1,
            line_end: 3,
            visibility: visibility.map(ToString::to_string),
        }
    }

    fn route(id: &str, file_path: &str, method: &str, path: &str) -> StoredRoute {
        StoredRoute {
            id: id.to_string(),
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            method: method.to_string(),
            path: path.to_string(),
            framework: "axum".to_string(),
            route_kind: "api".to_string(),
            file_path: file_path.to_string(),
            symbol_id: format!("{id}-symbol"),
            handler_name: Some("handler".to_string()),
            class_name: None,
            function_name: Some("handler".to_string()),
            line_start: 1,
            line_end: 2,
            confidence: 9000,
            source_kind: "RouteMetadata".to_string(),
        }
    }

    fn component(id: &str, file_path: &str, name: &str) -> StoredComponent {
        StoredComponent {
            id: id.to_string(),
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            name: name.to_string(),
            framework: "react".to_string(),
            file_path: file_path.to_string(),
            symbol_id: format!("{id}-symbol"),
            export_kind: Some("default".to_string()),
            component_kind: "function".to_string(),
            props_type_name: None,
            hooks: Vec::new(),
            usages: Vec::new(),
            line_start: 1,
            line_end: 5,
            confidence: 8500,
            source_kind: "ReactComponent".to_string(),
        }
    }

    fn data_access(id: &str, file_path: &str, entity: &str) -> StoredDataAccess {
        StoredDataAccess {
            id: id.to_string(),
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            technology: "sql".to_string(),
            kind: "TableReference".to_string(),
            operation: Some("read".to_string()),
            file_path: file_path.to_string(),
            symbol_id: format!("{id}-symbol"),
            class_name: None,
            method_name: None,
            entity_name: Some(entity.to_string()),
            context_name: None,
            repository_name: None,
            query_text: None,
            line_start: 1,
            line_end: 1,
            confidence: 7000,
            source_kind: "SqlTableReference".to_string(),
        }
    }

    fn messaging(id: &str, file_path: &str, topic: &str) -> StoredMessaging {
        StoredMessaging {
            id: id.to_string(),
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            technology: "kafka".to_string(),
            kind: "Producer".to_string(),
            direction: "outbound".to_string(),
            topic: Some(topic.to_string()),
            queue: None,
            exchange: None,
            routing_key: None,
            pattern: None,
            consumer_group: None,
            file_path: file_path.to_string(),
            symbol_id: format!("{id}-symbol"),
            class_name: None,
            function_name: Some("publish".to_string()),
            method_name: Some("publish".to_string()),
            line_start: 1,
            line_end: 1,
            confidence: 9000,
            source_kind: "KafkaProducerSend".to_string(),
        }
    }

    fn freshness(status: GitIndexFreshnessStatus) -> GitIndexFreshness {
        let is_fresh = matches!(status, GitIndexFreshnessStatus::Fresh);
        let is_stale = matches!(status, GitIndexFreshnessStatus::Stale);
        let manual_action_required = matches!(
            status,
            GitIndexFreshnessStatus::Stale
                | GitIndexFreshnessStatus::Unsafe
                | GitIndexFreshnessStatus::Unknown
        );
        GitIndexFreshness {
            status,
            is_stale,
            reindex_recommended: !is_fresh,
            manual_action_required,
            auto_reindex_allowed: false,
            auto_reindex_mode: AutoIndexMode::None,
            stale_reasons: vec![GitStaleReason::CommitChanged],
            current: Some(b3_core::GitRepositoryStatus {
                is_git_repo: true,
                repo_root: None,
                git_dir: None,
                current_branch: Some("main".to_string()),
                detached_head: false,
                head_commit: Some("abc".to_string()),
                short_head_commit: Some("abc".to_string()),
                working_tree: GitWorkingTreeStatus::default(),
                warnings: Vec::new(),
            }),
            indexed: None,
            auto_index_decision: AutoIndexDecision {
                allowed: false,
                mode: AutoIndexMode::None,
                blocked_reasons: Vec::new(),
                requires_manual_action: false,
                recommendation: "manual reindex if needed".to_string(),
            },
            warnings: Vec::new(),
            recommendation: "manual reindex recommended".to_string(),
        }
    }
}
