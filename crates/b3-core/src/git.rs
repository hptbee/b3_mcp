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
}
