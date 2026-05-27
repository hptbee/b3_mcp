//! Local read-only Git status detection.
//!
//! This crate owns Git command interaction for Phase 21.1. It only runs bounded
//! local read-only commands and returns warnings instead of failing callers.

mod cli;
mod status;

pub use b3_core::{
    GitReaderConfig, GitRepositoryStatus, GitStatusError, GitStatusErrorKind, GitWorkingTreeStatus,
};
pub use status::{parse_porcelain_status, read_git_status, READ_ONLY_GIT_COMMANDS};
