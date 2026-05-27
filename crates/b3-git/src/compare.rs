use crate::{cli::run_git, diff_summary::parse_numstat, read_git_status};
use b3_core::{
    GitBranchInfo, GitChangedFile, GitChangedFileStatus, GitCompareDiffMode, GitCompareRequest,
    GitCompareResult, GitDiffSummary, GitReaderConfig,
};
use std::{collections::BTreeMap, path::Path};

pub const READ_ONLY_COMPARE_COMMANDS: &[&[&str]] = &[
    &["rev-parse", "<ref>"],
    &["merge-base", "<base>", "<head>"],
    &["diff", "--name-status", "<range>"],
    &["diff", "--numstat", "<range>"],
    &["branch", "--list"],
];

pub fn read_branch_compare(project_root: &Path, request: GitCompareRequest) -> GitCompareResult {
    let reader_config = GitReaderConfig {
        command_timeout_ms: request.command_timeout_ms,
        max_stdout_bytes: request.max_stdout_bytes,
        allow_git_cli: true,
        allow_direct_git_fallback: false,
    };
    let status = read_git_status(project_root, reader_config);
    if !status.is_git_repo {
        return GitCompareResult {
            is_git_repo: false,
            repo_root: None,
            base_ref: request.base_ref,
            base_commit: None,
            head_ref: request.head_ref,
            head_commit: None,
            merge_base: None,
            diff_mode: request.diff_mode,
            diff_summary: None,
            warnings: status.warnings,
            truncated: false,
        };
    }

    let mut warnings = status.warnings;
    let base_ref = request
        .base_ref
        .clone()
        .or_else(|| default_base_ref(project_root, reader_config, &mut warnings));
    let head_ref = request
        .head_ref
        .clone()
        .unwrap_or_else(|| "HEAD".to_string());

    let Some(base_ref_value) = base_ref.clone() else {
        warnings.push("no_base_ref_found".to_string());
        return GitCompareResult {
            is_git_repo: true,
            repo_root: status
                .repo_root
                .map(|path| path.to_string_lossy().to_string()),
            base_ref: None,
            base_commit: None,
            head_ref: Some(head_ref),
            head_commit: None,
            merge_base: None,
            diff_mode: request.diff_mode,
            diff_summary: None,
            warnings,
            truncated: false,
        };
    };

    let base_commit = resolve_ref(project_root, &base_ref_value, reader_config, &mut warnings);
    let head_commit = resolve_ref(project_root, &head_ref, reader_config, &mut warnings);
    let merge_base = if request.diff_mode == GitCompareDiffMode::MergeBaseTripleDot {
        match run_git(
            project_root,
            &["merge-base", &base_ref_value, &head_ref],
            reader_config,
        ) {
            Ok(output) => Some(output.trim().to_string()).filter(|value| !value.is_empty()),
            Err(error) => {
                warnings.push(format!("merge-base could not be read: {}", error.message));
                None
            }
        }
    } else {
        None
    };

    let range = compare_range(&base_ref_value, &head_ref, request.diff_mode);
    let mut changed_files = match run_git(
        project_root,
        &["diff", "--name-status", range.as_str()],
        reader_config,
    ) {
        Ok(output) => parse_name_status(&output),
        Err(error) => {
            warnings.push(format!(
                "compare name-status could not be read: {}",
                error.message
            ));
            Vec::new()
        }
    };

    if request.include_line_counts {
        match run_git(
            project_root,
            &["diff", "--numstat", range.as_str()],
            reader_config,
        ) {
            Ok(output) => apply_line_counts(&mut changed_files, &parse_numstat(&output)),
            Err(error) => warnings.push(format!(
                "compare numstat could not be read: {}",
                error.message
            )),
        }
    }

    if request.include_untracked {
        warnings.push(
            "untracked files are working-tree state and are not included in branch compare"
                .to_string(),
        );
    }

    let mut truncated = false;
    if changed_files.len() > request.max_changed_files {
        changed_files.truncate(request.max_changed_files);
        truncated = true;
        warnings.push(format!(
            "compare diff output truncated to {} file(s)",
            request.max_changed_files
        ));
    }

    let mut diff_summary = build_compare_summary(
        status
            .repo_root
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        Some(base_ref_value.clone()),
        Some(head_ref.clone()),
        changed_files,
        truncated,
        Vec::new(),
    );
    diff_summary.warnings.extend(warnings.clone());

    GitCompareResult {
        is_git_repo: true,
        repo_root: status
            .repo_root
            .map(|path| path.to_string_lossy().to_string()),
        base_ref: Some(base_ref_value),
        base_commit,
        head_ref: Some(head_ref),
        head_commit,
        merge_base,
        diff_mode: request.diff_mode,
        diff_summary: Some(diff_summary),
        warnings,
        truncated,
    }
}

pub fn parse_branch_list(output: &str) -> Vec<GitBranchInfo> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let is_current = line.starts_with('*');
            let name = trimmed.trim_start_matches("* ").trim().to_string();
            Some(GitBranchInfo {
                full_ref: Some(format!("refs/heads/{name}")),
                commit: None,
                short_commit: None,
                is_current,
                is_detached: name.contains("HEAD detached") || name == "HEAD",
                is_remote_tracking: false,
                warnings: Vec::new(),
                name,
            })
        })
        .collect()
}

pub fn parse_name_status(output: &str) -> Vec<GitChangedFile> {
    output.lines().filter_map(parse_name_status_line).collect()
}

fn parse_name_status_line(line: &str) -> Option<GitChangedFile> {
    let mut parts = line.split('\t');
    let status_token = parts.next()?;
    let status_char = status_token.chars().next().unwrap_or('?');
    let status = match status_char {
        'A' => GitChangedFileStatus::Added,
        'M' => GitChangedFileStatus::Modified,
        'D' => GitChangedFileStatus::Deleted,
        'R' => GitChangedFileStatus::Renamed,
        'C' => GitChangedFileStatus::Copied,
        'T' => GitChangedFileStatus::TypeChanged,
        _ => GitChangedFileStatus::Unknown,
    };
    let first_path = parts.next()?.to_string();
    let (path, old_path) = if matches!(
        status,
        GitChangedFileStatus::Renamed | GitChangedFileStatus::Copied
    ) {
        let new_path = parts.next().unwrap_or(first_path.as_str()).to_string();
        (new_path, Some(first_path))
    } else {
        (first_path, None)
    };
    Some(GitChangedFile {
        path,
        old_path,
        status,
        staged: false,
        unstaged: false,
        untracked: false,
        conflicted: false,
        lines_added: None,
        lines_deleted: None,
        language: None,
        is_indexed: None,
        warnings: Vec::new(),
    })
}

fn default_base_ref(
    project_root: &Path,
    config: GitReaderConfig,
    warnings: &mut Vec<String>,
) -> Option<String> {
    match run_git(project_root, &["branch", "--list"], config) {
        Ok(output) => {
            let branches = parse_branch_list(&output);
            if branches.iter().any(|branch| branch.name == "main") {
                Some("main".to_string())
            } else if branches.iter().any(|branch| branch.name == "master") {
                Some("master".to_string())
            } else {
                warnings.push("no_base_ref_found".to_string());
                None
            }
        }
        Err(error) => {
            warnings.push(format!("branch list could not be read: {}", error.message));
            None
        }
    }
}

fn resolve_ref(
    project_root: &Path,
    git_ref: &str,
    config: GitReaderConfig,
    warnings: &mut Vec<String>,
) -> Option<String> {
    match run_git(project_root, &["rev-parse", git_ref], config) {
        Ok(output) => Some(output.trim().to_string()).filter(|value| !value.is_empty()),
        Err(error) => {
            warnings.push(format!(
                "ref {git_ref} could not be resolved: {}",
                error.message
            ));
            None
        }
    }
}

fn compare_range(base_ref: &str, head_ref: &str, mode: GitCompareDiffMode) -> String {
    match mode {
        GitCompareDiffMode::MergeBaseTripleDot => format!("{base_ref}...{head_ref}"),
        GitCompareDiffMode::DirectDoubleDot => format!("{base_ref}..{head_ref}"),
    }
}

fn apply_line_counts(
    changed_files: &mut [GitChangedFile],
    counts: &BTreeMap<String, (Option<u64>, Option<u64>)>,
) {
    for file in changed_files {
        if let Some((added, deleted)) = counts.get(&file.path) {
            file.lines_added = *added;
            file.lines_deleted = *deleted;
        }
    }
}

fn build_compare_summary(
    repo_root: Option<String>,
    base_ref: Option<String>,
    head_ref: Option<String>,
    changed_files: Vec<GitChangedFile>,
    truncated: bool,
    warnings: Vec<String>,
) -> GitDiffSummary {
    let mut summary = GitDiffSummary {
        is_git_repo: true,
        repo_root,
        base_ref,
        head_ref,
        changed_files,
        staged_count: 0,
        unstaged_count: 0,
        untracked_count: 0,
        conflicted_count: 0,
        added_count: 0,
        modified_count: 0,
        deleted_count: 0,
        renamed_count: 0,
        copied_count: 0,
        total_changed_count: 0,
        total_lines_added: Some(0),
        total_lines_deleted: Some(0),
        truncated,
        warnings,
    };

    for file in &summary.changed_files {
        match file.status {
            GitChangedFileStatus::Added => summary.added_count += 1,
            GitChangedFileStatus::Modified => summary.modified_count += 1,
            GitChangedFileStatus::Deleted => summary.deleted_count += 1,
            GitChangedFileStatus::Renamed => summary.renamed_count += 1,
            GitChangedFileStatus::Copied => summary.copied_count += 1,
            _ => {}
        }
        summary.total_lines_added = combine_count(summary.total_lines_added, file.lines_added);
        summary.total_lines_deleted =
            combine_count(summary.total_lines_deleted, file.lines_deleted);
    }
    summary.total_changed_count = summary.changed_files.len();
    summary
}

fn combine_count(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_list_with_current_marker() {
        let branches = parse_branch_list("  main\n* feature/git\n  master\n");

        assert_eq!(branches.len(), 3);
        assert!(branches[1].is_current);
        assert_eq!(branches[1].name, "feature/git");
        assert!(!branches.iter().any(|branch| branch.is_remote_tracking));
    }

    #[test]
    fn parses_added_modified_deleted_renamed_copied_and_typechanged() {
        let files = parse_name_status(
            "A\tsrc/new.rs\nM\tsrc/lib.rs\nD\tsrc/old.rs\nR100\tsrc/a.rs\tsrc/b.rs\nC100\tsrc/c.rs\tsrc/d.rs\nT\tasset.bin\n",
        );

        assert_eq!(files.len(), 6);
        assert_eq!(files[0].status, GitChangedFileStatus::Added);
        assert_eq!(files[1].status, GitChangedFileStatus::Modified);
        assert_eq!(files[2].status, GitChangedFileStatus::Deleted);
        assert_eq!(files[3].status, GitChangedFileStatus::Renamed);
        assert_eq!(files[3].old_path.as_deref(), Some("src/a.rs"));
        assert_eq!(files[3].path, "src/b.rs");
        assert_eq!(files[4].status, GitChangedFileStatus::Copied);
        assert_eq!(files[5].status, GitChangedFileStatus::TypeChanged);
    }

    #[test]
    fn parses_paths_with_spaces_from_tab_separated_name_status() {
        let files = parse_name_status("M\tsrc/file with spaces.rs\n");

        assert_eq!(files[0].path, "src/file with spaces.rs");
    }

    #[test]
    fn applies_numstat_counts_and_binary_markers() {
        let mut files = parse_name_status("M\tsrc/lib.rs\nM\tassets/logo.png\n");
        let counts = parse_numstat("10\t2\tsrc/lib.rs\n-\t-\tassets/logo.png\n");

        apply_line_counts(&mut files, &counts);

        assert_eq!(files[0].lines_added, Some(10));
        assert_eq!(files[0].lines_deleted, Some(2));
        assert_eq!(files[1].lines_added, None);
        assert_eq!(files[1].lines_deleted, None);
    }

    #[test]
    fn compare_summary_counts_and_truncation_are_bounded() {
        let mut files = parse_name_status("M\ta.rs\nA\tb.rs\nD\tc.rs\n");
        files.truncate(2);
        let summary = build_compare_summary(
            Some("repo".to_string()),
            Some("main".to_string()),
            Some("HEAD".to_string()),
            files,
            true,
            vec!["truncated".to_string()],
        );

        assert!(summary.truncated);
        assert_eq!(summary.total_changed_count, 2);
        assert_eq!(summary.modified_count, 1);
        assert_eq!(summary.added_count, 1);
    }

    #[test]
    fn read_only_compare_command_list_excludes_mutating_and_remote_commands() {
        let forbidden = [
            "checkout", "switch", "commit", "merge", "rebase", "reset", "clean", "push", "pull",
            "fetch", "tag", "-D",
        ];

        for command in READ_ONLY_COMPARE_COMMANDS {
            for token in *command {
                assert!(
                    !forbidden.contains(token),
                    "forbidden git command token present: {token}"
                );
            }
        }
    }
}
