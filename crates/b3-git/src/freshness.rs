use b3_core::{
    AutoIndexDecision, AutoIndexMode, AutoIndexPolicy, AutoIndexPolicyMode, GitChangedFileStatus,
    GitDiffSummary, GitIndexFreshness, GitIndexFreshnessStatus, GitIndexSnapshot,
    GitRepositoryStatus, GitStaleReason,
};

pub fn evaluate_git_index_freshness(
    current: Option<GitRepositoryStatus>,
    indexed: Option<GitIndexSnapshot>,
    policy: AutoIndexPolicy,
) -> GitIndexFreshness {
    evaluate_git_index_freshness_with_diff(current, indexed, policy, None)
}

pub fn evaluate_git_index_freshness_with_diff(
    current: Option<GitRepositoryStatus>,
    indexed: Option<GitIndexSnapshot>,
    policy: AutoIndexPolicy,
    diff_summary: Option<&GitDiffSummary>,
) -> GitIndexFreshness {
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();

    let Some(current_status) = current else {
        reasons.push(GitStaleReason::NoGitStatus);
        let decision = AutoIndexDecision::blocked(
            vec!["unknown_git_state".to_string()],
            true,
            "Git status is unavailable; inspect the repository and reindex manually if needed.",
        );
        return GitIndexFreshness {
            status: GitIndexFreshnessStatus::Unknown,
            is_stale: false,
            reindex_recommended: false,
            manual_action_required: true,
            auto_reindex_allowed: false,
            auto_reindex_mode: AutoIndexMode::None,
            stale_reasons: reasons,
            current: None,
            indexed,
            auto_index_decision: decision,
            warnings,
            recommendation: "Git status is unavailable; inspect the repository before reindexing."
                .to_string(),
        };
    };

    if !current_status.warnings.is_empty() {
        reasons.push(GitStaleReason::GitStatusWarning);
        warnings.extend(current_status.warnings.clone());
    }

    let Some(indexed_snapshot) = indexed else {
        reasons.push(GitStaleReason::NoIndexSnapshot);
        let decision = AutoIndexDecision::blocked(
            vec!["no_index_snapshot".to_string()],
            true,
            "No indexed Git snapshot exists; run a manual reindex to establish a baseline.",
        );
        return GitIndexFreshness {
            status: GitIndexFreshnessStatus::Stale,
            is_stale: true,
            reindex_recommended: true,
            manual_action_required: true,
            auto_reindex_allowed: false,
            auto_reindex_mode: AutoIndexMode::None,
            stale_reasons: reasons,
            current: Some(current_status),
            indexed: None,
            auto_index_decision: decision,
            warnings,
            recommendation: "Manual reindex recommended to record a Git index snapshot."
                .to_string(),
        };
    };

    if !indexed_snapshot.git_status_warnings.is_empty() {
        reasons.push(GitStaleReason::SnapshotWarning);
        warnings.extend(indexed_snapshot.git_status_warnings.clone());
    }
    if let Some(diff) = diff_summary {
        if diff.truncated {
            reasons.push(GitStaleReason::ExcessiveChangedFiles);
            warnings.push("changed-file list was truncated".to_string());
        }
        if diff
            .changed_files
            .iter()
            .any(|file| !is_policy_safe_status(file.status, &policy))
        {
            reasons.push(GitStaleReason::UnsafeChangeShape);
        }
    }

    classify_reasons(&current_status, &indexed_snapshot, &mut reasons);
    let status = classify_status(&current_status, &indexed_snapshot, &reasons);
    let is_stale = matches!(status, GitIndexFreshnessStatus::Stale);
    let reindex_recommended = is_stale
        || matches!(status, GitIndexFreshnessStatus::Dirty)
        || reasons.contains(&GitStaleReason::NoIndexSnapshot);
    let manual_action_required = matches!(
        status,
        GitIndexFreshnessStatus::Stale
            | GitIndexFreshnessStatus::Unsafe
            | GitIndexFreshnessStatus::Unknown
    );
    let decision = evaluate_auto_index_policy_with_diff(
        &current_status,
        &indexed_snapshot,
        &policy,
        diff_summary,
    );
    let auto_reindex_allowed = decision.allowed;
    let auto_reindex_mode = decision.mode;
    let recommendation = recommendation_for(&status, &reasons, &decision);

    GitIndexFreshness {
        status,
        is_stale,
        reindex_recommended,
        manual_action_required: manual_action_required || decision.requires_manual_action,
        auto_reindex_allowed,
        auto_reindex_mode,
        stale_reasons: reasons,
        current: Some(current_status),
        indexed: Some(indexed_snapshot),
        auto_index_decision: decision,
        warnings,
        recommendation,
    }
}

pub fn evaluate_auto_index_policy(
    current: &GitRepositoryStatus,
    indexed: &GitIndexSnapshot,
    policy: &AutoIndexPolicy,
) -> AutoIndexDecision {
    evaluate_auto_index_policy_with_diff(current, indexed, policy, None)
}

pub fn evaluate_auto_index_policy_with_diff(
    current: &GitRepositoryStatus,
    indexed: &GitIndexSnapshot,
    policy: &AutoIndexPolicy,
    diff_summary: Option<&GitDiffSummary>,
) -> AutoIndexDecision {
    let mut blocked = Vec::new();
    let mut manual = false;

    if !policy.enabled || policy.mode == AutoIndexPolicyMode::Off {
        blocked.push("auto_index_disabled".to_string());
    }
    if policy.require_known_git_state && !current.is_git_repo {
        blocked.push("no_git_repo".to_string());
        manual = true;
    }
    if policy.require_known_git_state && !indexed.is_git_repo {
        blocked.push("indexed_snapshot_not_git".to_string());
        manual = true;
    }
    if policy.require_same_branch && current.current_branch != indexed.indexed_branch {
        blocked.push("branch_changed".to_string());
        manual = true;
    }
    if policy.require_same_commit && current.head_commit != indexed.indexed_commit {
        blocked.push("commit_changed".to_string());
        manual = true;
    }
    if current.detached_head || indexed.indexed_detached_head {
        blocked.push("detached_head".to_string());
        manual = true;
    }
    if policy.require_no_conflicts
        && (current.working_tree.conflicted_count > 0 || indexed.indexed_conflicted_count > 0)
    {
        blocked.push("conflict_detected".to_string());
        manual = true;
    }
    if current.working_tree.total_changed_count > policy.max_changed_files {
        blocked.push("excessive_changed_files".to_string());
        manual = true;
    }
    if current.warnings.is_empty() {
        // no-op; explicit branch keeps warning handling below easy to scan.
    } else {
        blocked.push("git_status_warning".to_string());
    }
    let changed_file_list_available = policy.changed_file_list_available || diff_summary.is_some();
    if !changed_file_list_available && current.working_tree.dirty {
        blocked.push("changed_file_list_not_available_until_phase_21_4".to_string());
    }
    if let Some(diff) = diff_summary {
        if diff.truncated {
            blocked.push("truncated_changed_files".to_string());
            manual = true;
        }
        if diff.total_changed_count > policy.max_changed_files {
            blocked.push("excessive_changed_files".to_string());
            manual = true;
        }
        for file in &diff.changed_files {
            if !is_policy_safe_status(file.status, policy) {
                blocked.push(format!("unsafe_change_status:{:?}", file.status));
                manual = true;
            }
        }
    }

    if !blocked.is_empty() {
        return AutoIndexDecision::blocked(
            blocked,
            manual,
            "Auto-index is blocked; use manual reindex guidance or wait for changed-file details.",
        );
    }

    AutoIndexDecision {
        allowed: current.working_tree.dirty,
        mode: if current.working_tree.dirty {
            AutoIndexMode::IncrementalChangedFiles
        } else {
            AutoIndexMode::None
        },
        blocked_reasons: Vec::new(),
        requires_manual_action: false,
        recommendation: if current.working_tree.dirty {
            "Conservative incremental changed-file indexing is allowed by policy.".to_string()
        } else {
            "Index is clean; no auto-index action is needed.".to_string()
        },
    }
}

fn is_policy_safe_status(status: GitChangedFileStatus, policy: &AutoIndexPolicy) -> bool {
    match status {
        GitChangedFileStatus::Modified | GitChangedFileStatus::Added => true,
        GitChangedFileStatus::Untracked => policy.allow_untracked,
        GitChangedFileStatus::Deleted => policy.allow_deleted,
        GitChangedFileStatus::Renamed => policy.allow_renamed,
        GitChangedFileStatus::Copied => policy.allow_copied,
        GitChangedFileStatus::TypeChanged => policy.allow_type_changed,
        GitChangedFileStatus::Conflicted | GitChangedFileStatus::Unknown => false,
    }
}

fn classify_reasons(
    current: &GitRepositoryStatus,
    indexed: &GitIndexSnapshot,
    reasons: &mut Vec<GitStaleReason>,
) {
    if !current.is_git_repo || !indexed.is_git_repo {
        reasons.push(GitStaleReason::NoGitRepo);
    }
    if current
        .repo_root
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
        != indexed.git_repo_root
    {
        reasons.push(GitStaleReason::RepoRootChanged);
    }
    if current.current_branch != indexed.indexed_branch {
        reasons.push(GitStaleReason::BranchChanged);
    }
    if current.head_commit != indexed.indexed_commit {
        reasons.push(GitStaleReason::CommitChanged);
    }
    if current.detached_head || indexed.indexed_detached_head {
        reasons.push(GitStaleReason::DetachedHead);
    }
    if current.working_tree.dirty {
        reasons.push(GitStaleReason::WorkingTreeDirty);
    }
    if current.working_tree.conflicted_count > 0 {
        reasons.push(GitStaleReason::ConflictDetected);
    }
    if indexed.indexed_conflicted_count > 0 {
        reasons.push(GitStaleReason::IndexedConflicted);
    }
}

fn classify_status(
    current: &GitRepositoryStatus,
    indexed: &GitIndexSnapshot,
    reasons: &[GitStaleReason],
) -> GitIndexFreshnessStatus {
    if !current.is_git_repo || !indexed.is_git_repo {
        return GitIndexFreshnessStatus::Unknown;
    }
    if reasons.iter().any(|reason| {
        matches!(
            reason,
            GitStaleReason::DetachedHead
                | GitStaleReason::ConflictDetected
                | GitStaleReason::IndexedConflicted
                | GitStaleReason::GitStatusWarning
                | GitStaleReason::ExcessiveChangedFiles
                | GitStaleReason::UnsafeChangeShape
        )
    }) {
        return GitIndexFreshnessStatus::Unsafe;
    }
    if reasons.iter().any(|reason| {
        matches!(
            reason,
            GitStaleReason::RepoRootChanged
                | GitStaleReason::BranchChanged
                | GitStaleReason::CommitChanged
        )
    }) {
        return GitIndexFreshnessStatus::Stale;
    }
    if current.working_tree.dirty {
        return GitIndexFreshnessStatus::Dirty;
    }
    GitIndexFreshnessStatus::Fresh
}

fn recommendation_for(
    status: &GitIndexFreshnessStatus,
    reasons: &[GitStaleReason],
    decision: &AutoIndexDecision,
) -> String {
    if reasons.contains(&GitStaleReason::BranchChanged)
        || reasons.contains(&GitStaleReason::CommitChanged)
    {
        return "Branch or commit changed; run a manual reindex. Auto full reindex is blocked."
            .to_string();
    }
    match status {
        GitIndexFreshnessStatus::Fresh => {
            "Index matches the current Git branch and commit.".to_string()
        }
        GitIndexFreshnessStatus::Dirty => {
            if decision.allowed {
                "Working tree is dirty; conservative incremental indexing is allowed by policy."
                    .to_string()
            } else {
                "Working tree is dirty; manual or later changed-file reindex is recommended."
                    .to_string()
            }
        }
        GitIndexFreshnessStatus::Stale => {
            "Index is stale; run a manual reindex to refresh branch-aware metadata.".to_string()
        }
        GitIndexFreshnessStatus::Unsafe => {
            "Git state is unsafe for auto-index; resolve conflicts or inspect manually.".to_string()
        }
        GitIndexFreshnessStatus::Unknown => {
            "Git freshness is unknown; inspect repository state before reindexing.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{BranchId, GitChangedFile, GitWorkingTreeStatus, ProjectId};
    use std::path::PathBuf;

    fn clean_status() -> GitRepositoryStatus {
        GitRepositoryStatus {
            is_git_repo: true,
            repo_root: Some(PathBuf::from("D:/repo")),
            git_dir: Some(PathBuf::from("D:/repo/.git")),
            current_branch: Some("main".to_string()),
            detached_head: false,
            head_commit: Some("abc123".to_string()),
            short_head_commit: Some("abc123".to_string()),
            working_tree: GitWorkingTreeStatus::default(),
            warnings: Vec::new(),
        }
    }

    fn snapshot() -> GitIndexSnapshot {
        GitIndexSnapshot::from_status(
            ProjectId::new("project"),
            BranchId::new("main"),
            clean_status(),
            1,
        )
    }

    #[test]
    fn fresh_when_branch_commit_and_repo_match() {
        let result = evaluate_git_index_freshness(
            Some(clean_status()),
            Some(snapshot()),
            AutoIndexPolicy::default(),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Fresh);
        assert!(!result.is_stale);
        assert!(!result.reindex_recommended);
        assert!(!result.auto_reindex_allowed);
    }

    #[test]
    fn dirty_same_commit_recommends_reindex_but_default_auto_disabled() {
        let mut current = clean_status();
        current.working_tree = GitWorkingTreeStatus {
            dirty: true,
            staged_count: 1,
            total_changed_count: 1,
            ..GitWorkingTreeStatus::default()
        };

        let result = evaluate_git_index_freshness(
            Some(current),
            Some(snapshot()),
            AutoIndexPolicy::default(),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Dirty);
        assert!(result.reindex_recommended);
        assert!(!result.manual_action_required);
        assert!(!result.auto_reindex_allowed);
        assert!(result
            .auto_index_decision
            .blocked_reasons
            .contains(&"auto_index_disabled".to_string()));
    }

    #[test]
    fn enabled_policy_can_allow_bounded_dirty_when_changed_files_available() {
        let mut current = clean_status();
        current.working_tree = GitWorkingTreeStatus {
            dirty: true,
            unstaged_count: 2,
            total_changed_count: 2,
            ..GitWorkingTreeStatus::default()
        };
        let mut policy = AutoIndexPolicy::conservative_enabled();
        policy.changed_file_list_available = true;

        let result = evaluate_git_index_freshness(Some(current), Some(snapshot()), policy);

        assert_eq!(result.status, GitIndexFreshnessStatus::Dirty);
        assert!(result.auto_reindex_allowed);
        assert_eq!(
            result.auto_reindex_mode,
            AutoIndexMode::IncrementalChangedFiles
        );
    }

    #[test]
    fn branch_change_is_stale_manual_and_blocks_auto() {
        let mut current = clean_status();
        current.current_branch = Some("feature".to_string());
        let mut policy = AutoIndexPolicy::conservative_enabled();
        policy.changed_file_list_available = true;

        let result = evaluate_git_index_freshness(Some(current), Some(snapshot()), policy);

        assert_eq!(result.status, GitIndexFreshnessStatus::Stale);
        assert!(result.is_stale);
        assert!(result.manual_action_required);
        assert!(!result.auto_reindex_allowed);
        assert!(result
            .stale_reasons
            .contains(&GitStaleReason::BranchChanged));
    }

    #[test]
    fn commit_change_is_stale_manual_and_blocks_auto() {
        let mut current = clean_status();
        current.head_commit = Some("def456".to_string());

        let result = evaluate_git_index_freshness(
            Some(current),
            Some(snapshot()),
            AutoIndexPolicy::conservative_enabled(),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Stale);
        assert!(result.manual_action_required);
        assert!(!result.auto_reindex_allowed);
        assert!(result
            .stale_reasons
            .contains(&GitStaleReason::CommitChanged));
    }

    #[test]
    fn missing_snapshot_is_stale_and_manual() {
        let result = evaluate_git_index_freshness(
            Some(clean_status()),
            None,
            AutoIndexPolicy::conservative_enabled(),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Stale);
        assert!(result.reindex_recommended);
        assert!(result.manual_action_required);
        assert!(result
            .stale_reasons
            .contains(&GitStaleReason::NoIndexSnapshot));
    }

    #[test]
    fn detached_head_is_unsafe_and_blocks_auto() {
        let mut current = clean_status();
        current.current_branch = None;
        current.detached_head = true;

        let result = evaluate_git_index_freshness(
            Some(current),
            Some(snapshot()),
            AutoIndexPolicy::conservative_enabled(),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Unsafe);
        assert!(result.manual_action_required);
        assert!(!result.auto_reindex_allowed);
        assert!(result.stale_reasons.contains(&GitStaleReason::DetachedHead));
    }

    #[test]
    fn conflicts_are_unsafe_and_block_auto() {
        let mut current = clean_status();
        current.working_tree = GitWorkingTreeStatus {
            dirty: true,
            conflicted_count: 1,
            total_changed_count: 1,
            ..GitWorkingTreeStatus::default()
        };

        let result = evaluate_git_index_freshness(
            Some(current),
            Some(snapshot()),
            AutoIndexPolicy::conservative_enabled(),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Unsafe);
        assert!(result.manual_action_required);
        assert!(!result.auto_reindex_allowed);
        assert!(result
            .stale_reasons
            .contains(&GitStaleReason::ConflictDetected));
    }

    #[test]
    fn unknown_status_blocks_auto() {
        let result =
            evaluate_git_index_freshness(None, Some(snapshot()), AutoIndexPolicy::default());

        assert_eq!(result.status, GitIndexFreshnessStatus::Unknown);
        assert!(result.manual_action_required);
        assert!(!result.auto_reindex_allowed);
        assert!(result.stale_reasons.contains(&GitStaleReason::NoGitStatus));
    }

    #[test]
    fn excessive_changes_block_auto() {
        let mut current = clean_status();
        current.working_tree = GitWorkingTreeStatus {
            dirty: true,
            unstaged_count: 30,
            total_changed_count: 30,
            ..GitWorkingTreeStatus::default()
        };
        let mut policy = AutoIndexPolicy::conservative_enabled();
        policy.max_changed_files = 2;
        policy.changed_file_list_available = true;

        let result = evaluate_git_index_freshness(Some(current), Some(snapshot()), policy);

        assert_eq!(result.status, GitIndexFreshnessStatus::Dirty);
        assert!(!result.auto_reindex_allowed);
        assert!(result
            .auto_index_decision
            .blocked_reasons
            .contains(&"excessive_changed_files".to_string()));
    }

    #[test]
    fn changed_file_diff_allows_safe_modified_added_when_policy_enabled() {
        let mut current = clean_status();
        current.working_tree = GitWorkingTreeStatus {
            dirty: true,
            staged_count: 1,
            unstaged_count: 1,
            total_changed_count: 2,
            ..GitWorkingTreeStatus::default()
        };
        let mut policy = AutoIndexPolicy::conservative_enabled();
        policy.changed_file_list_available = true;
        let diff = diff_with(vec![
            changed_file("src/lib.rs", GitChangedFileStatus::Modified),
            changed_file("src/new.rs", GitChangedFileStatus::Added),
        ]);

        let result = evaluate_git_index_freshness_with_diff(
            Some(current),
            Some(snapshot()),
            policy,
            Some(&diff),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Dirty);
        assert!(result.auto_reindex_allowed);
    }

    #[test]
    fn changed_file_diff_blocks_truncated_deleted_renamed_and_unknown_by_default() {
        let mut current = clean_status();
        current.working_tree = GitWorkingTreeStatus {
            dirty: true,
            staged_count: 3,
            total_changed_count: 3,
            ..GitWorkingTreeStatus::default()
        };
        let mut policy = AutoIndexPolicy::conservative_enabled();
        policy.changed_file_list_available = true;
        let mut diff = diff_with(vec![
            changed_file("deleted.rs", GitChangedFileStatus::Deleted),
            changed_file("renamed.rs", GitChangedFileStatus::Renamed),
            changed_file("mystery.rs", GitChangedFileStatus::Unknown),
        ]);
        diff.truncated = true;

        let result = evaluate_git_index_freshness_with_diff(
            Some(current),
            Some(snapshot()),
            policy,
            Some(&diff),
        );

        assert_eq!(result.status, GitIndexFreshnessStatus::Unsafe);
        assert!(!result.auto_reindex_allowed);
        assert!(result
            .auto_index_decision
            .blocked_reasons
            .contains(&"truncated_changed_files".to_string()));
        assert!(result
            .stale_reasons
            .contains(&GitStaleReason::UnsafeChangeShape));
    }

    fn changed_file(path: &str, status: GitChangedFileStatus) -> GitChangedFile {
        GitChangedFile {
            path: path.to_string(),
            old_path: None,
            status,
            staged: true,
            unstaged: false,
            untracked: status == GitChangedFileStatus::Untracked,
            conflicted: status == GitChangedFileStatus::Conflicted,
            lines_added: Some(1),
            lines_deleted: Some(0),
            language: None,
            is_indexed: None,
            warnings: Vec::new(),
        }
    }

    fn diff_with(changed_files: Vec<GitChangedFile>) -> GitDiffSummary {
        GitDiffSummary {
            is_git_repo: true,
            repo_root: Some("D:/repo".to_string()),
            base_ref: None,
            head_ref: Some("abc123".to_string()),
            staged_count: changed_files.len(),
            unstaged_count: 0,
            untracked_count: 0,
            conflicted_count: 0,
            added_count: 0,
            modified_count: 0,
            deleted_count: 0,
            renamed_count: 0,
            copied_count: 0,
            total_changed_count: changed_files.len(),
            total_lines_added: Some(changed_files.len() as u64),
            total_lines_deleted: Some(0),
            truncated: false,
            warnings: Vec::new(),
            changed_files,
        }
    }
}
