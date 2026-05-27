use crate::cli::run_git;
use b3_core::{GitReaderConfig, GitRepositoryStatus, GitStatusError, GitWorkingTreeStatus};
use std::path::{Path, PathBuf};

pub const READ_ONLY_GIT_COMMANDS: &[&[&str]] = &[
    &["rev-parse", "--show-toplevel"],
    &["rev-parse", "--absolute-git-dir"],
    &["rev-parse", "--abbrev-ref", "HEAD"],
    &["rev-parse", "HEAD"],
    &["status", "--porcelain=v1", "--branch"],
];

pub fn read_git_status(project_root: &Path, config: GitReaderConfig) -> GitRepositoryStatus {
    let mut warnings = Vec::new();

    let repo_root = match run_required_git(project_root, &["rev-parse", "--show-toplevel"], config)
    {
        Ok(value) => PathBuf::from(value),
        Err(err) => {
            return GitRepositoryStatus::not_git_repo(format!(
                "Git repository was not detected: {}",
                err.message
            ));
        }
    };

    let git_dir = match run_required_git(project_root, &["rev-parse", "--absolute-git-dir"], config)
    {
        Ok(value) => Some(PathBuf::from(value)),
        Err(err) => {
            warnings.push(format!("Git directory could not be read: {}", err.message));
            None
        }
    };

    let branch_output =
        run_required_git(project_root, &["rev-parse", "--abbrev-ref", "HEAD"], config);
    let mut detached_head = false;
    let current_branch = match branch_output {
        Ok(value) if value == "HEAD" => {
            detached_head = true;
            None
        }
        Ok(value) if value.is_empty() => None,
        Ok(value) => Some(value),
        Err(err) => {
            warnings.push(format!("Git branch could not be read: {}", err.message));
            None
        }
    };

    let head_commit = match run_required_git(project_root, &["rev-parse", "HEAD"], config) {
        Ok(value) if value.is_empty() => None,
        Ok(value) => Some(value),
        Err(err) => {
            warnings.push(format!(
                "Git HEAD commit could not be read: {}",
                err.message
            ));
            None
        }
    };
    let short_head_commit = head_commit.as_ref().map(|commit| {
        let len = commit.len().min(12);
        commit[..len].to_string()
    });

    let working_tree = match run_required_git(
        project_root,
        &["status", "--porcelain=v1", "--branch"],
        config,
    ) {
        Ok(output) => parse_porcelain_status(&output),
        Err(err) => {
            warnings.push(format!(
                "Git working tree status could not be read: {}",
                err.message
            ));
            GitWorkingTreeStatus::default()
        }
    };

    if working_tree.conflicted_count > 0 {
        warnings.push(format!(
            "Git working tree has {} conflicted path(s)",
            working_tree.conflicted_count
        ));
    }

    GitRepositoryStatus {
        is_git_repo: true,
        repo_root: Some(repo_root),
        git_dir,
        current_branch,
        detached_head,
        head_commit,
        short_head_commit,
        working_tree,
        warnings,
    }
}

pub fn parse_porcelain_status(output: &str) -> GitWorkingTreeStatus {
    let mut status = GitWorkingTreeStatus::default();

    for line in output.lines() {
        if line.starts_with("##") || line.trim().is_empty() {
            continue;
        }

        let bytes = line.as_bytes();
        if bytes.len() < 2 {
            continue;
        }

        let index = bytes[0] as char;
        let worktree = bytes[1] as char;
        let conflict = is_conflict_status(index, worktree);

        status.total_changed_count += 1;

        if index == '?' && worktree == '?' {
            status.untracked_count += 1;
            continue;
        }

        if conflict {
            status.conflicted_count += 1;
            continue;
        }

        if index != ' ' && index != '?' {
            status.staged_count += 1;
        }

        if worktree != ' ' && worktree != '?' {
            status.unstaged_count += 1;
        }
    }

    status.dirty = status.total_changed_count > 0;
    status
}

fn run_required_git(
    project_root: &Path,
    args: &[&str],
    config: GitReaderConfig,
) -> Result<String, GitStatusError> {
    run_git(project_root, args, config).map(|output| output.trim().to_string())
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
    use std::{fs, process::Command};
    use tempfile::tempdir;

    #[test]
    fn parse_clean_branch_status() {
        let parsed = parse_porcelain_status("## main\n");

        assert!(!parsed.dirty);
        assert_eq!(parsed.total_changed_count, 0);
    }

    #[test]
    fn parse_dirty_branch_counts_staged_unstaged_and_untracked() {
        let parsed = parse_porcelain_status(
            "## main...origin/main [ahead 1]\n M src/lib.rs\nM  README.md\nMM Cargo.toml\nA  src/new.rs\n?? scratch.txt\n",
        );

        assert!(parsed.dirty);
        assert_eq!(parsed.staged_count, 3);
        assert_eq!(parsed.unstaged_count, 2);
        assert_eq!(parsed.untracked_count, 1);
        assert_eq!(parsed.conflicted_count, 0);
        assert_eq!(parsed.total_changed_count, 5);
    }

    #[test]
    fn parse_conflicts_without_double_counting_staged_or_unstaged() {
        let parsed =
            parse_porcelain_status("## main\nUU a.rs\nAA b.rs\nDD c.rs\nAU d.rs\nUA e.rs\n");

        assert!(parsed.dirty);
        assert_eq!(parsed.staged_count, 0);
        assert_eq!(parsed.unstaged_count, 0);
        assert_eq!(parsed.untracked_count, 0);
        assert_eq!(parsed.conflicted_count, 5);
        assert_eq!(parsed.total_changed_count, 5);
    }

    #[test]
    fn parse_detached_head_header_with_no_changes() {
        let parsed = parse_porcelain_status("## HEAD (no branch)\n");

        assert!(!parsed.dirty);
        assert_eq!(parsed.total_changed_count, 0);
    }

    #[test]
    fn no_git_project_returns_safe_status() {
        let dir = tempdir().expect("tempdir");
        let status = read_git_status(dir.path(), GitReaderConfig::default());

        assert!(!status.is_git_repo);
        assert!(status.repo_root.is_none());
        assert!(!status.warnings.is_empty());
    }

    #[test]
    fn git_cli_disabled_returns_warning_not_panic() {
        let dir = tempdir().expect("tempdir");
        let status = read_git_status(
            dir.path(),
            GitReaderConfig {
                allow_git_cli: false,
                ..GitReaderConfig::default()
            },
        );

        assert!(!status.is_git_repo);
        assert!(status
            .warnings
            .iter()
            .any(|warning| warning.contains("disabled")));
    }

    #[test]
    fn allowed_command_list_contains_only_read_only_commands() {
        let forbidden = [
            "checkout", "switch", "commit", "merge", "rebase", "reset", "clean", "push", "pull",
            "fetch", "branch", "tag",
        ];

        for command in READ_ONLY_GIT_COMMANDS {
            for token in *command {
                assert!(
                    !forbidden.contains(token),
                    "forbidden git command token present: {token}"
                );
            }
        }
    }

    #[test]
    fn read_status_from_subdirectory_when_git_is_available() {
        let dir = tempdir().expect("tempdir");
        if Command::new("git")
            .arg("--version")
            .output()
            .map(|output| !output.status.success())
            .unwrap_or(true)
        {
            return;
        }

        let init = Command::new("git")
            .arg("init")
            .current_dir(dir.path())
            .output()
            .expect("git init should run when git is available");
        if !init.status.success() {
            return;
        }

        let nested = dir.path().join("src").join("nested");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(dir.path().join("tracked.txt"), "hello").expect("write file");

        let status = read_git_status(&nested, GitReaderConfig::default());

        assert!(status.is_git_repo);
        assert_eq!(status.repo_root.as_deref(), Some(dir.path()));
        assert!(status.working_tree.dirty);
        assert_eq!(status.working_tree.untracked_count, 1);
    }
}
