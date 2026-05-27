//! Local Git Intelligence contracts.
//!
//! These DTOs are implementation-light by design. Concrete Git access lives in
//! a reader crate so core contracts never execute commands or mutate state.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{BranchId, ProjectId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRepositoryStatus {
    pub is_git_repo: bool,
    pub repo_root: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub current_branch: Option<String>,
    pub detached_head: bool,
    pub head_commit: Option<String>,
    pub short_head_commit: Option<String>,
    pub working_tree: GitWorkingTreeStatus,
    pub warnings: Vec<String>,
}

impl GitRepositoryStatus {
    pub fn not_git_repo(warning: impl Into<String>) -> Self {
        Self {
            is_git_repo: false,
            repo_root: None,
            git_dir: None,
            current_branch: None,
            detached_head: false,
            head_commit: None,
            short_head_commit: None,
            working_tree: GitWorkingTreeStatus::default(),
            warnings: vec![warning.into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GitWorkingTreeStatus {
    pub dirty: bool,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub conflicted_count: usize,
    pub total_changed_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitReaderConfig {
    pub command_timeout_ms: u64,
    pub max_stdout_bytes: usize,
    pub allow_git_cli: bool,
    pub allow_direct_git_fallback: bool,
}

impl Default for GitReaderConfig {
    fn default() -> Self {
        Self {
            command_timeout_ms: 2_000,
            max_stdout_bytes: 512 * 1024,
            allow_git_cli: true,
            allow_direct_git_fallback: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitStatusErrorKind {
    GitNotFound,
    NotGitRepo,
    CommandFailed,
    TimedOut,
    InvalidUtf8,
    OutputTooLarge,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusError {
    pub kind: GitStatusErrorKind,
    pub message: String,
}

impl GitStatusError {
    pub fn new(kind: GitStatusErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitIndexSnapshot {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub is_git_repo: bool,
    pub git_repo_root: Option<String>,
    pub git_dir: Option<String>,
    pub indexed_branch: Option<String>,
    pub indexed_commit: Option<String>,
    pub indexed_short_commit: Option<String>,
    pub indexed_detached_head: bool,
    pub indexed_dirty: bool,
    pub indexed_staged_count: usize,
    pub indexed_unstaged_count: usize,
    pub indexed_untracked_count: usize,
    pub indexed_conflicted_count: usize,
    pub indexed_total_changed_count: usize,
    pub indexed_at_unix_ms: u64,
    pub git_status_warnings: Vec<String>,
}

impl GitIndexSnapshot {
    pub fn from_status(
        project_id: ProjectId,
        branch_id: BranchId,
        status: GitRepositoryStatus,
        indexed_at_unix_ms: u64,
    ) -> Self {
        Self {
            project_id,
            branch_id,
            is_git_repo: status.is_git_repo,
            git_repo_root: status
                .repo_root
                .map(|path| path.to_string_lossy().to_string()),
            git_dir: status
                .git_dir
                .map(|path| path.to_string_lossy().to_string()),
            indexed_branch: status.current_branch,
            indexed_commit: status.head_commit,
            indexed_short_commit: status.short_head_commit,
            indexed_detached_head: status.detached_head,
            indexed_dirty: status.working_tree.dirty,
            indexed_staged_count: status.working_tree.staged_count,
            indexed_unstaged_count: status.working_tree.unstaged_count,
            indexed_untracked_count: status.working_tree.untracked_count,
            indexed_conflicted_count: status.working_tree.conflicted_count,
            indexed_total_changed_count: status.working_tree.total_changed_count,
            indexed_at_unix_ms,
            git_status_warnings: status.warnings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitIndexFreshnessStatus {
    Fresh,
    Dirty,
    Stale,
    Unsafe,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitStaleReason {
    NoGitStatus,
    NoIndexSnapshot,
    NoGitRepo,
    RepoRootChanged,
    BranchChanged,
    CommitChanged,
    DetachedHead,
    WorkingTreeDirty,
    ConflictDetected,
    IndexedConflicted,
    GitStatusWarning,
    SnapshotWarning,
    ExcessiveChangedFiles,
    UnsafeChangeShape,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitIndexFreshness {
    pub status: GitIndexFreshnessStatus,
    pub is_stale: bool,
    pub reindex_recommended: bool,
    pub manual_action_required: bool,
    pub auto_reindex_allowed: bool,
    pub auto_reindex_mode: AutoIndexMode,
    pub stale_reasons: Vec<GitStaleReason>,
    pub current: Option<GitRepositoryStatus>,
    pub indexed: Option<GitIndexSnapshot>,
    pub auto_index_decision: AutoIndexDecision,
    pub warnings: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoIndexPolicyMode {
    Off,
    Conservative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoIndexMode {
    None,
    IncrementalChangedFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoIndexPolicy {
    pub enabled: bool,
    pub mode: AutoIndexPolicyMode,
    pub max_changed_files: usize,
    pub require_same_branch: bool,
    pub require_same_commit: bool,
    pub require_no_conflicts: bool,
    pub require_known_git_state: bool,
    pub allow_untracked: bool,
    pub allow_deleted: bool,
    pub allow_renamed: bool,
    pub allow_copied: bool,
    pub allow_type_changed: bool,
    pub changed_file_list_available: bool,
}

impl Default for AutoIndexPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: AutoIndexPolicyMode::Off,
            max_changed_files: 25,
            require_same_branch: true,
            require_same_commit: true,
            require_no_conflicts: true,
            require_known_git_state: true,
            allow_untracked: true,
            allow_deleted: false,
            allow_renamed: false,
            allow_copied: false,
            allow_type_changed: false,
            changed_file_list_available: false,
        }
    }
}

impl AutoIndexPolicy {
    pub fn conservative_enabled() -> Self {
        Self {
            enabled: true,
            mode: AutoIndexPolicyMode::Conservative,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoIndexDecision {
    pub allowed: bool,
    pub mode: AutoIndexMode,
    pub blocked_reasons: Vec<String>,
    pub requires_manual_action: bool,
    pub recommendation: String,
}

impl AutoIndexDecision {
    pub fn blocked(
        blocked_reasons: Vec<String>,
        requires_manual_action: bool,
        recommendation: impl Into<String>,
    ) -> Self {
        Self {
            allowed: false,
            mode: AutoIndexMode::None,
            blocked_reasons,
            requires_manual_action,
            recommendation: recommendation.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: GitChangedFileStatus,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
    pub lines_added: Option<u64>,
    pub lines_deleted: Option<u64>,
    pub language: Option<String>,
    pub is_indexed: Option<bool>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitChangedFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
    TypeChanged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffSummary {
    pub is_git_repo: bool,
    pub repo_root: Option<String>,
    pub base_ref: Option<String>,
    pub head_ref: Option<String>,
    pub changed_files: Vec<GitChangedFile>,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub conflicted_count: usize,
    pub added_count: usize,
    pub modified_count: usize,
    pub deleted_count: usize,
    pub renamed_count: usize,
    pub copied_count: usize,
    pub total_changed_count: usize,
    pub total_lines_added: Option<u64>,
    pub total_lines_deleted: Option<u64>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffSummaryConfig {
    pub max_changed_files: usize,
    pub max_stdout_bytes: usize,
    pub command_timeout_ms: u64,
    pub include_untracked: bool,
    pub include_line_counts: bool,
    pub allow_numstat: bool,
    pub allow_name_status: bool,
}

impl Default for GitDiffSummaryConfig {
    fn default() -> Self {
        Self {
            max_changed_files: 100,
            max_stdout_bytes: 512 * 1024,
            command_timeout_ms: 2_000,
            include_untracked: true,
            include_line_counts: true,
            allow_numstat: true,
            allow_name_status: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchInfo {
    pub name: String,
    pub full_ref: Option<String>,
    pub commit: Option<String>,
    pub short_commit: Option<String>,
    pub is_current: bool,
    pub is_detached: bool,
    pub is_remote_tracking: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitCompareDiffMode {
    MergeBaseTripleDot,
    DirectDoubleDot,
}

impl Default for GitCompareDiffMode {
    fn default() -> Self {
        Self::MergeBaseTripleDot
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCompareRequest {
    pub base_ref: Option<String>,
    pub head_ref: Option<String>,
    pub diff_mode: GitCompareDiffMode,
    pub include_line_counts: bool,
    pub include_untracked: bool,
    pub max_changed_files: usize,
    pub max_stdout_bytes: usize,
    pub command_timeout_ms: u64,
}

impl Default for GitCompareRequest {
    fn default() -> Self {
        Self {
            base_ref: None,
            head_ref: None,
            diff_mode: GitCompareDiffMode::default(),
            include_line_counts: true,
            include_untracked: false,
            max_changed_files: 100,
            max_stdout_bytes: 512 * 1024,
            command_timeout_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCompareResult {
    pub is_git_repo: bool,
    pub repo_root: Option<String>,
    pub base_ref: Option<String>,
    pub base_commit: Option<String>,
    pub head_ref: Option<String>,
    pub head_commit: Option<String>,
    pub merge_base: Option<String>,
    pub diff_mode: GitCompareDiffMode,
    pub diff_summary: Option<GitDiffSummary>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_index_snapshot_serializes_no_git_optional_fields() {
        let snapshot = GitIndexSnapshot {
            project_id: ProjectId::new("project"),
            branch_id: BranchId::new("main"),
            is_git_repo: false,
            git_repo_root: None,
            git_dir: None,
            indexed_branch: None,
            indexed_commit: None,
            indexed_short_commit: None,
            indexed_detached_head: false,
            indexed_dirty: false,
            indexed_staged_count: 0,
            indexed_unstaged_count: 0,
            indexed_untracked_count: 0,
            indexed_conflicted_count: 0,
            indexed_total_changed_count: 0,
            indexed_at_unix_ms: 42,
            git_status_warnings: vec!["not a git repository".to_string()],
        };

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: GitIndexSnapshot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, snapshot);
        assert!(!decoded.is_git_repo);
        assert!(decoded.indexed_branch.is_none());
    }

    #[test]
    fn git_compare_request_and_result_serialize() {
        let request = GitCompareRequest {
            base_ref: Some("main".to_string()),
            head_ref: Some("HEAD".to_string()),
            diff_mode: GitCompareDiffMode::MergeBaseTripleDot,
            ..GitCompareRequest::default()
        };
        let json = serde_json::to_string(&request).expect("serialize request");
        let decoded: GitCompareRequest = serde_json::from_str(&json).expect("decode request");
        assert_eq!(decoded.diff_mode, GitCompareDiffMode::MergeBaseTripleDot);

        let result = GitCompareResult {
            is_git_repo: true,
            repo_root: Some("repo".to_string()),
            base_ref: Some("main".to_string()),
            base_commit: Some("abc".to_string()),
            head_ref: Some("HEAD".to_string()),
            head_commit: Some("def".to_string()),
            merge_base: Some("abc".to_string()),
            diff_mode: GitCompareDiffMode::DirectDoubleDot,
            diff_summary: None,
            warnings: Vec::new(),
            truncated: false,
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        let decoded: GitCompareResult = serde_json::from_str(&json).expect("decode result");
        assert_eq!(decoded.base_ref.as_deref(), Some("main"));
        assert_eq!(decoded.diff_mode, GitCompareDiffMode::DirectDoubleDot);
    }
}
