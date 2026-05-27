//! Local read-only Git status detection.
//!
//! This crate owns Git command interaction for Phase 21.1. It only runs bounded
//! local read-only commands and returns warnings instead of failing callers.

mod cli;
mod compare;
mod diff_summary;
mod freshness;
mod status;

pub use b3_core::{
    AutoIndexDecision, AutoIndexMode, AutoIndexPolicy, AutoIndexPolicyMode, GitBranchInfo,
    GitChangedFile, GitChangedFileStatus, GitCompareDiffMode, GitCompareRequest, GitCompareResult,
    GitDiffSummary, GitDiffSummaryConfig, GitIndexFreshness, GitIndexFreshnessStatus,
    GitIndexSnapshot, GitReaderConfig, GitRepositoryStatus, GitStaleReason, GitStatusError,
    GitStatusErrorKind, GitWorkingTreeStatus,
};
pub use compare::{
    parse_branch_list, parse_name_status, read_branch_compare, READ_ONLY_COMPARE_COMMANDS,
};
pub use diff_summary::{
    parse_numstat, parse_porcelain_z_changed_files, read_diff_summary, READ_ONLY_DIFF_COMMANDS,
};
pub use freshness::{
    evaluate_auto_index_policy, evaluate_auto_index_policy_with_diff, evaluate_git_index_freshness,
    evaluate_git_index_freshness_with_diff,
};
pub use status::{parse_porcelain_status, read_git_status, READ_ONLY_GIT_COMMANDS};
