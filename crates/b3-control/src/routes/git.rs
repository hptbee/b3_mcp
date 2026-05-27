use super::*;
use b3_core::{
    AutoIndexPolicy, AutoIndexPolicyMode, GitCompareRequest, GitDiffSummaryConfig,
    GitIndexSnapshot, GitReaderConfig,
};
use b3_git::{
    evaluate_git_index_freshness_with_diff, read_branch_compare, read_diff_summary,
    read_git_status, read_local_branches,
};
use b3_query::git_impact::{
    analyze_git_compare_impact, analyze_git_diff_impact, GitDiffImpactRequest, ImpactProfile,
};

const DEFAULT_GIT_MAX_CHANGED_FILES: usize = 200;
const HARD_GIT_MAX_CHANGED_FILES: usize = 1_000;
const DEFAULT_GIT_MAX_IMPACTED_ITEMS: usize = 200;
const HARD_GIT_MAX_IMPACTED_ITEMS: usize = 1_000;
const DEFAULT_GIT_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const HARD_GIT_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const DEFAULT_GIT_TIMEOUT_MS: u64 = 2_000;
const HARD_GIT_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct GitReadQuery {
    max_changed_files: Option<usize>,
    include_untracked: Option<bool>,
    include_line_counts: Option<bool>,
    max_stdout_bytes: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct GitFreshnessQuery {
    max_changed_files: Option<usize>,
    include_untracked: Option<bool>,
    include_line_counts: Option<bool>,
    max_stdout_bytes: Option<usize>,
    timeout_ms: Option<u64>,
    auto_index_mode: Option<String>,
    allow_untracked: Option<bool>,
    allow_deleted: Option<bool>,
    allow_renamed: Option<bool>,
    allow_copied: Option<bool>,
    allow_type_changed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GitImpactMode {
    WorkingTree,
    Compare,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitImpactControlRequest {
    mode: Option<GitImpactMode>,
    compare_request: Option<GitCompareRequest>,
    include_untracked: Option<bool>,
    include_line_counts: Option<bool>,
    include_architecture: Option<bool>,
    include_context_seeds: Option<bool>,
    max_changed_files: Option<usize>,
    max_impacted_items: Option<usize>,
    max_stdout_bytes: Option<usize>,
    timeout_ms: Option<u64>,
    profile: Option<ImpactProfile>,
}

pub(crate) async fn git_status(
    State(state): State<ControlState>,
    Query(query): Query<GitReadQuery>,
) -> Json<Value> {
    Json(json!(read_git_status(
        &state.project_path,
        reader_config(&query)
    )))
}

pub(crate) async fn git_freshness(
    State(state): State<ControlState>,
    Query(query): Query<GitFreshnessQuery>,
) -> Result<Json<Value>, ControlError> {
    let read_query = GitReadQuery {
        max_changed_files: query.max_changed_files,
        include_untracked: query.include_untracked,
        include_line_counts: query.include_line_counts,
        max_stdout_bytes: query.max_stdout_bytes,
        timeout_ms: query.timeout_ms,
    };
    let status = read_git_status(&state.project_path, reader_config(&read_query));
    let diff_summary = read_diff_summary(&state.project_path, diff_config(&read_query));
    let snapshot = latest_snapshot(&state).await?;
    let freshness = evaluate_git_index_freshness_with_diff(
        Some(status),
        snapshot,
        auto_index_policy(&query),
        Some(&diff_summary),
    );

    Ok(Json(json!(freshness)))
}

pub(crate) async fn git_changed_files(
    State(state): State<ControlState>,
    Query(query): Query<GitReadQuery>,
) -> Json<Value> {
    Json(json!(read_diff_summary(
        &state.project_path,
        diff_config(&query)
    )))
}

pub(crate) async fn git_diff_summary(
    State(state): State<ControlState>,
    Query(query): Query<GitReadQuery>,
) -> Json<Value> {
    Json(json!(read_diff_summary(
        &state.project_path,
        diff_config(&query)
    )))
}

pub(crate) async fn git_branches(
    State(state): State<ControlState>,
    Query(query): Query<GitReadQuery>,
) -> Json<Value> {
    let (is_git_repo, repo_root, branches, warnings) =
        read_local_branches(&state.project_path, reader_config(&query));
    let status = read_git_status(&state.project_path, reader_config(&query));
    Json(json!({
        "is_git_repo": is_git_repo,
        "repo_root": repo_root,
        "current_branch": status.current_branch,
        "detached_head": status.detached_head,
        "branches": branches,
        "warnings": warnings
    }))
}

pub(crate) async fn git_compare(
    State(state): State<ControlState>,
    payload: Result<Json<GitCompareRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    let result = read_branch_compare(&state.project_path, bounded_compare_request(request));

    Ok(Json(json!(result)))
}

pub(crate) async fn git_impact(
    State(state): State<ControlState>,
    payload: Result<Json<GitImpactControlRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    let read_query = GitReadQuery {
        max_changed_files: request.max_changed_files,
        include_untracked: request.include_untracked,
        include_line_counts: request.include_line_counts,
        max_stdout_bytes: request.max_stdout_bytes,
        timeout_ms: request.timeout_ms,
    };
    let freshness = git_freshness_value(&state, &read_query).await?;
    let impact_request = impact_request(&state, &request);
    let mode = request.mode.unwrap_or(GitImpactMode::WorkingTree);

    let response = match mode {
        GitImpactMode::WorkingTree => {
            let diff_summary = read_diff_summary(&state.project_path, diff_config(&read_query));
            let storage = state.storage.lock().await;
            let impact =
                analyze_git_diff_impact(&*storage, &impact_request, Some(freshness), diff_summary)
                    .map_err(ControlError::internal)?;
            json!({
                "mode": "working_tree",
                "impact": impact
            })
        }
        GitImpactMode::Compare => {
            let compare_request = request.compare_request.unwrap_or_default();
            let compare = read_branch_compare(
                &state.project_path,
                bounded_compare_request(compare_request),
            );
            let storage = state.storage.lock().await;
            let impact =
                analyze_git_compare_impact(&*storage, &impact_request, Some(freshness), &compare)
                    .map_err(ControlError::internal)?;
            json!({
                "mode": "compare",
                "compare": compare,
                "impact": impact
            })
        }
    };

    Ok(Json(response))
}

async fn git_freshness_value(
    state: &ControlState,
    query: &GitReadQuery,
) -> Result<b3_core::GitIndexFreshness, ControlError> {
    let status = read_git_status(&state.project_path, reader_config(query));
    let diff_summary = read_diff_summary(&state.project_path, diff_config(query));
    let snapshot = latest_snapshot(state).await?;
    Ok(evaluate_git_index_freshness_with_diff(
        Some(status),
        snapshot,
        AutoIndexPolicy::default(),
        Some(&diff_summary),
    ))
}

async fn latest_snapshot(state: &ControlState) -> Result<Option<GitIndexSnapshot>, ControlError> {
    let storage = state.storage.lock().await;
    storage
        .latest_git_index_snapshot(&ProjectId::new("default"), &BranchId::new("main"))
        .map_err(ControlError::internal)
}

fn reader_config(query: &GitReadQuery) -> GitReaderConfig {
    GitReaderConfig {
        command_timeout_ms: query
            .timeout_ms
            .unwrap_or(DEFAULT_GIT_TIMEOUT_MS)
            .min(HARD_GIT_TIMEOUT_MS),
        max_stdout_bytes: query
            .max_stdout_bytes
            .unwrap_or(DEFAULT_GIT_MAX_STDOUT_BYTES)
            .clamp(1, HARD_GIT_MAX_STDOUT_BYTES),
        allow_git_cli: true,
        allow_direct_git_fallback: false,
    }
}

fn diff_config(query: &GitReadQuery) -> GitDiffSummaryConfig {
    GitDiffSummaryConfig {
        max_changed_files: query
            .max_changed_files
            .unwrap_or(DEFAULT_GIT_MAX_CHANGED_FILES)
            .clamp(1, HARD_GIT_MAX_CHANGED_FILES),
        max_stdout_bytes: query
            .max_stdout_bytes
            .unwrap_or(DEFAULT_GIT_MAX_STDOUT_BYTES)
            .clamp(1, HARD_GIT_MAX_STDOUT_BYTES),
        command_timeout_ms: query
            .timeout_ms
            .unwrap_or(DEFAULT_GIT_TIMEOUT_MS)
            .min(HARD_GIT_TIMEOUT_MS),
        include_untracked: query.include_untracked.unwrap_or(true),
        include_line_counts: query.include_line_counts.unwrap_or(true),
        allow_numstat: true,
        allow_name_status: true,
    }
}

fn auto_index_policy(query: &GitFreshnessQuery) -> AutoIndexPolicy {
    let enabled = matches!(query.auto_index_mode.as_deref(), Some("conservative"));
    AutoIndexPolicy {
        enabled,
        mode: if enabled {
            AutoIndexPolicyMode::Conservative
        } else {
            AutoIndexPolicyMode::Off
        },
        max_changed_files: query
            .max_changed_files
            .unwrap_or(DEFAULT_GIT_MAX_CHANGED_FILES)
            .clamp(1, HARD_GIT_MAX_CHANGED_FILES),
        allow_untracked: query.allow_untracked.unwrap_or(true),
        allow_deleted: query.allow_deleted.unwrap_or(false),
        allow_renamed: query.allow_renamed.unwrap_or(false),
        allow_copied: query.allow_copied.unwrap_or(false),
        allow_type_changed: query.allow_type_changed.unwrap_or(false),
        changed_file_list_available: true,
        ..AutoIndexPolicy::default()
    }
}

fn bounded_compare_request(mut request: GitCompareRequest) -> GitCompareRequest {
    request.max_changed_files = request
        .max_changed_files
        .clamp(1, HARD_GIT_MAX_CHANGED_FILES);
    request.max_stdout_bytes = request.max_stdout_bytes.clamp(1, HARD_GIT_MAX_STDOUT_BYTES);
    request.command_timeout_ms = request.command_timeout_ms.min(HARD_GIT_TIMEOUT_MS);
    request
}

fn impact_request(state: &ControlState, request: &GitImpactControlRequest) -> GitDiffImpactRequest {
    GitDiffImpactRequest {
        project_id: "default".to_string(),
        branch_id: "main".to_string(),
        project_path: Some(path_string(&state.project_path)),
        database_path: Some(path_string(&state.database_path)),
        include_untracked: request.include_untracked.unwrap_or(true),
        include_line_counts: request.include_line_counts.unwrap_or(true),
        profile: request.profile.unwrap_or(ImpactProfile::Balanced),
        max_changed_files: request
            .max_changed_files
            .unwrap_or(DEFAULT_GIT_MAX_CHANGED_FILES)
            .clamp(1, HARD_GIT_MAX_CHANGED_FILES),
        max_impacted_items: request
            .max_impacted_items
            .unwrap_or(DEFAULT_GIT_MAX_IMPACTED_ITEMS)
            .clamp(1, HARD_GIT_MAX_IMPACTED_ITEMS),
        include_context_pack: request.include_context_seeds.unwrap_or(true),
        include_architecture: request.include_architecture.unwrap_or(false),
    }
}
