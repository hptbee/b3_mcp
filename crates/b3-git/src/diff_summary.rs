use crate::{cli::run_git, read_git_status};
use b3_core::{
    GitChangedFile, GitChangedFileStatus, GitDiffSummary, GitDiffSummaryConfig, GitReaderConfig,
};
use std::{collections::BTreeMap, path::Path};

pub const READ_ONLY_DIFF_COMMANDS: &[&[&str]] = &[
    &["status", "--porcelain=v1", "-z", "--branch"],
    &["diff", "--numstat"],
    &["diff", "--cached", "--numstat"],
    &["diff", "--name-status"],
    &["diff", "--cached", "--name-status"],
    &["ls-files", "--others", "--exclude-standard"],
];

pub fn read_diff_summary(project_root: &Path, config: GitDiffSummaryConfig) -> GitDiffSummary {
    let reader_config = GitReaderConfig {
        command_timeout_ms: config.command_timeout_ms,
        max_stdout_bytes: config.max_stdout_bytes,
        allow_git_cli: true,
        allow_direct_git_fallback: false,
    };
    let status = read_git_status(project_root, reader_config);
    if !status.is_git_repo {
        return GitDiffSummary {
            is_git_repo: false,
            repo_root: None,
            base_ref: None,
            head_ref: None,
            changed_files: Vec::new(),
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
            truncated: false,
            warnings: status.warnings,
        };
    }

    let mut warnings = status.warnings;
    let status_output = match run_git(
        project_root,
        &["status", "--porcelain=v1", "-z", "--branch"],
        reader_config,
    ) {
        Ok(output) => output,
        Err(error) => {
            warnings.push(format!(
                "Git changed-file status could not be read: {}",
                error.message
            ));
            String::new()
        }
    };
    let mut changed_files =
        parse_porcelain_z_changed_files(&status_output, config.include_untracked);

    if config.include_line_counts && config.allow_numstat {
        let mut line_counts = BTreeMap::new();
        collect_numstat(
            project_root,
            reader_config,
            &["diff", "--numstat"],
            &mut line_counts,
            &mut warnings,
        );
        collect_numstat(
            project_root,
            reader_config,
            &["diff", "--cached", "--numstat"],
            &mut line_counts,
            &mut warnings,
        );
        apply_line_counts(&mut changed_files, &line_counts);
    }

    let mut truncated = false;
    if changed_files.len() > config.max_changed_files {
        changed_files.truncate(config.max_changed_files);
        truncated = true;
        warnings.push(format!(
            "changed file output truncated to {} file(s)",
            config.max_changed_files
        ));
    }

    build_summary(
        status
            .repo_root
            .map(|path| path.to_string_lossy().to_string()),
        status.head_commit,
        changed_files,
        truncated,
        warnings,
    )
}

pub fn parse_porcelain_z_changed_files(
    output: &str,
    include_untracked: bool,
) -> Vec<GitChangedFile> {
    let mut files = Vec::new();
    let mut entries = output
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .peekable();

    while let Some(entry) = entries.next() {
        if entry.starts_with("##") || entry.len() < 3 {
            continue;
        }

        let bytes = entry.as_bytes();
        let index = bytes[0] as char;
        let worktree = bytes[1] as char;
        let path = entry[3..].to_string();
        let status = status_from_xy(index, worktree);
        let mut old_path = None;

        if matches!(
            status,
            GitChangedFileStatus::Renamed | GitChangedFileStatus::Copied
        ) {
            old_path = entries.next().map(str::to_string);
        }

        if status == GitChangedFileStatus::Untracked && !include_untracked {
            continue;
        }

        files.push(GitChangedFile {
            path,
            old_path,
            status,
            staged: index != ' ' && index != '?' && !is_conflict_status(index, worktree),
            unstaged: worktree != ' ' && worktree != '?' && !is_conflict_status(index, worktree),
            untracked: index == '?' && worktree == '?',
            conflicted: is_conflict_status(index, worktree),
            lines_added: None,
            lines_deleted: None,
            language: None,
            is_indexed: None,
            warnings: Vec::new(),
        });
    }

    files
}

pub fn parse_numstat(output: &str) -> BTreeMap<String, (Option<u64>, Option<u64>)> {
    let mut counts = BTreeMap::new();
    for line in output.lines() {
        let mut parts = line.splitn(3, '\t');
        let added = parts.next();
        let deleted = parts.next();
        let path = parts.next();
        let Some(path) = path else {
            continue;
        };
        let parsed = match (added, deleted) {
            (Some("-"), Some("-")) => (None, None),
            (Some(added), Some(deleted)) => match (added.parse::<u64>(), deleted.parse::<u64>()) {
                (Ok(added), Ok(deleted)) => (Some(added), Some(deleted)),
                _ => (None, None),
            },
            _ => (None, None),
        };
        merge_counts(&mut counts, path.to_string(), parsed);
    }
    counts
}

fn collect_numstat(
    project_root: &Path,
    reader_config: GitReaderConfig,
    args: &[&str],
    line_counts: &mut BTreeMap<String, (Option<u64>, Option<u64>)>,
    warnings: &mut Vec<String>,
) {
    match run_git(project_root, args, reader_config) {
        Ok(output) => {
            for (path, counts) in parse_numstat(&output) {
                merge_counts(line_counts, path, counts);
            }
        }
        Err(error) => warnings.push(format!("Git numstat could not be read: {}", error.message)),
    }
}

fn merge_counts(
    line_counts: &mut BTreeMap<String, (Option<u64>, Option<u64>)>,
    path: String,
    counts: (Option<u64>, Option<u64>),
) {
    line_counts
        .entry(path)
        .and_modify(|existing| {
            existing.0 = combine_count(existing.0, counts.0);
            existing.1 = combine_count(existing.1, counts.1);
        })
        .or_insert(counts);
}

fn combine_count(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn apply_line_counts(
    changed_files: &mut [GitChangedFile],
    line_counts: &BTreeMap<String, (Option<u64>, Option<u64>)>,
) {
    for file in changed_files {
        if let Some((added, deleted)) = line_counts.get(&file.path) {
            file.lines_added = *added;
            file.lines_deleted = *deleted;
        }
    }
}

fn build_summary(
    repo_root: Option<String>,
    head_ref: Option<String>,
    changed_files: Vec<GitChangedFile>,
    truncated: bool,
    warnings: Vec<String>,
) -> GitDiffSummary {
    let mut summary = GitDiffSummary {
        is_git_repo: true,
        repo_root,
        base_ref: None,
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
        if file.staged {
            summary.staged_count += 1;
        }
        if file.unstaged {
            summary.unstaged_count += 1;
        }
        if file.untracked {
            summary.untracked_count += 1;
        }
        if file.conflicted {
            summary.conflicted_count += 1;
        }
        match file.status {
            GitChangedFileStatus::Added => summary.added_count += 1,
            GitChangedFileStatus::Modified => summary.modified_count += 1,
            GitChangedFileStatus::Deleted => summary.deleted_count += 1,
            GitChangedFileStatus::Renamed => summary.renamed_count += 1,
            GitChangedFileStatus::Copied => summary.copied_count += 1,
            GitChangedFileStatus::Untracked => summary.added_count += 1,
            _ => {}
        }
        summary.total_lines_added = combine_count(summary.total_lines_added, file.lines_added);
        summary.total_lines_deleted =
            combine_count(summary.total_lines_deleted, file.lines_deleted);
    }

    summary.total_changed_count = summary.changed_files.len();
    summary
}

fn status_from_xy(index: char, worktree: char) -> GitChangedFileStatus {
    if index == '?' && worktree == '?' {
        return GitChangedFileStatus::Untracked;
    }
    if is_conflict_status(index, worktree) {
        return GitChangedFileStatus::Conflicted;
    }
    match first_meaningful_status(index, worktree) {
        'A' => GitChangedFileStatus::Added,
        'M' => GitChangedFileStatus::Modified,
        'D' => GitChangedFileStatus::Deleted,
        'R' => GitChangedFileStatus::Renamed,
        'C' => GitChangedFileStatus::Copied,
        'T' => GitChangedFileStatus::TypeChanged,
        _ => GitChangedFileStatus::Unknown,
    }
}

fn first_meaningful_status(index: char, worktree: char) -> char {
    if index != ' ' {
        index
    } else {
        worktree
    }
}

fn is_conflict_status(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_status() {
        assert!(parse_porcelain_z_changed_files("## main\0", true).is_empty());
    }

    #[test]
    fn parses_modified_staged_added_deleted_and_untracked() {
        let files = parse_porcelain_z_changed_files(
            "## main\0 M src/lib.rs\0M  Cargo.toml\0A  new file.rs\0 D old.rs\0?? scratch.txt\0",
            true,
        );

        assert_eq!(files.len(), 5);
        assert_eq!(files[0].status, GitChangedFileStatus::Modified);
        assert!(files[0].unstaged);
        assert_eq!(files[1].status, GitChangedFileStatus::Modified);
        assert!(files[1].staged);
        assert_eq!(files[2].status, GitChangedFileStatus::Added);
        assert_eq!(files[2].path, "new file.rs");
        assert_eq!(files[3].status, GitChangedFileStatus::Deleted);
        assert_eq!(files[4].status, GitChangedFileStatus::Untracked);
        assert!(files[4].untracked);
    }

    #[test]
    fn parses_renamed_copied_typechanged_and_conflicts() {
        let files = parse_porcelain_z_changed_files(
            "## HEAD (no branch)\0R  new.rs\0old.rs\0C  copy.rs\0source.rs\0 T kind.rs\0UU conflict.rs\0",
            true,
        );

        assert_eq!(files[0].status, GitChangedFileStatus::Renamed);
        assert_eq!(files[0].path, "new.rs");
        assert_eq!(files[0].old_path.as_deref(), Some("old.rs"));
        assert_eq!(files[1].status, GitChangedFileStatus::Copied);
        assert_eq!(files[1].old_path.as_deref(), Some("source.rs"));
        assert_eq!(files[2].status, GitChangedFileStatus::TypeChanged);
        assert_eq!(files[3].status, GitChangedFileStatus::Conflicted);
        assert!(files[3].conflicted);
    }

    #[test]
    fn can_exclude_untracked_files() {
        let files = parse_porcelain_z_changed_files("?? scratch.txt\0 M src/lib.rs\0", false);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/lib.rs");
    }

    #[test]
    fn parses_numstat_and_binary_markers() {
        let counts = parse_numstat("10\t2\tsrc/lib.rs\n-\t-\tassets/logo.png\n");

        assert_eq!(counts.get("src/lib.rs"), Some(&(Some(10), Some(2))));
        assert_eq!(counts.get("assets/logo.png"), Some(&(None, None)));
    }

    #[test]
    fn read_only_diff_command_list_excludes_mutating_commands() {
        let forbidden = [
            "checkout", "switch", "commit", "merge", "rebase", "reset", "clean", "push", "pull",
            "fetch", "branch", "tag",
        ];

        for command in READ_ONLY_DIFF_COMMANDS {
            for token in *command {
                assert!(
                    !forbidden.contains(token),
                    "forbidden git command token present: {token}"
                );
            }
        }
    }

    #[test]
    fn summary_truncates_changed_files() {
        let summary = build_summary(
            Some("repo".to_string()),
            Some("head".to_string()),
            parse_porcelain_z_changed_files(" M a.rs\0 M b.rs\0", true)
                .into_iter()
                .take(1)
                .collect(),
            true,
            vec!["truncated".to_string()],
        );

        assert!(summary.truncated);
        assert_eq!(summary.total_changed_count, 1);
        assert_eq!(summary.modified_count, 1);
    }
}
