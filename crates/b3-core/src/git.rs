//! Local Git Intelligence contracts.
//!
//! These DTOs are implementation-light by design. Concrete Git access lives in
//! a reader crate so core contracts never execute commands or mutate state.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
