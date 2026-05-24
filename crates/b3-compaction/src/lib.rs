//! Local command-output compaction.
//!
//! This crate never executes commands. It only summarizes stdout/stderr that a
//! caller already captured, using deterministic local string rules.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

const DEFAULT_MAX_BYTES: usize = 8_000;
const MAX_FINDINGS: usize = 20;
const MAX_FILES: usize = 20;
const MAX_SNIPPETS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandFamily {
    Git,
    Cargo,
    Dotnet,
    Npm,
    Pnpm,
    Yarn,
    Ng,
    Tsc,
    Eslint,
    Docker,
    DockerCompose,
    Rg,
    Grep,
    Cat,
    Tree,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    GitStatus,
    GitDiff,
    Cargo,
    Dotnet,
    Javascript,
    Docker,
    Search,
    FilePreview,
    Generic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutputInput {
    pub command: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub working_directory: Option<String>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutputSummary {
    pub compacted_output: String,
    pub key_findings: Vec<String>,
    pub warnings: Vec<CompactionWarning>,
    pub omitted_sections_count: usize,
    pub original_byte_estimate: usize,
    pub compacted_byte_estimate: usize,
    pub estimated_token_savings: usize,
    pub command_family: CommandFamily,
    pub strategy_used: CompactionStrategy,
    pub truncated: bool,
    pub metadata: CompactionMetadata,
}

pub type CompactionResult = CommandOutputSummary;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CompactionMetadata {
    pub line_count: usize,
    pub stderr_line_count: usize,
    pub exit_code: Option<i32>,
    pub max_bytes: usize,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionWarning {
    pub code: String,
    pub message: String,
}

pub trait CommandOutputCompactor {
    fn compact(&self, input: CommandOutputInput) -> CompactionResult;
}

#[derive(Debug, Clone, Default)]
pub struct LocalCommandOutputCompactor;

impl CommandOutputCompactor for LocalCommandOutputCompactor {
    fn compact(&self, input: CommandOutputInput) -> CompactionResult {
        compact_command_output(input)
    }
}

pub fn compact_command_output(input: CommandOutputInput) -> CompactionResult {
    let family = detect_command_family(&input.command, &input.argv);
    let command_lower = input.command.to_ascii_lowercase();
    let strategy = if family == CommandFamily::Git && command_lower.contains("status") {
        CompactionStrategy::GitStatus
    } else if family == CommandFamily::Git && command_lower.contains("diff") {
        CompactionStrategy::GitDiff
    } else {
        match family {
            CommandFamily::Cargo => CompactionStrategy::Cargo,
            CommandFamily::Dotnet => CompactionStrategy::Dotnet,
            CommandFamily::Npm
            | CommandFamily::Pnpm
            | CommandFamily::Yarn
            | CommandFamily::Ng
            | CommandFamily::Tsc
            | CommandFamily::Eslint => CompactionStrategy::Javascript,
            CommandFamily::Docker | CommandFamily::DockerCompose => CompactionStrategy::Docker,
            CommandFamily::Rg | CommandFamily::Grep => CompactionStrategy::Search,
            CommandFamily::Cat | CommandFamily::Tree => CompactionStrategy::FilePreview,
            _ => CompactionStrategy::Generic,
        }
    };

    let max_bytes = input.max_bytes.unwrap_or(DEFAULT_MAX_BYTES).max(256);
    let original_byte_estimate = input.stdout.len() + input.stderr.len();
    let mut metadata = CompactionMetadata {
        line_count: input.stdout.lines().count(),
        stderr_line_count: input.stderr.lines().count(),
        exit_code: input.exit_code,
        max_bytes,
        fields: BTreeMap::new(),
    };
    let mut warnings = Vec::new();
    if input.exit_code.unwrap_or(0) != 0 {
        warnings.push(warning(
            "non_zero_exit",
            "command exited with a non-zero status",
        ));
    }
    if !input.stderr.trim().is_empty() {
        warnings.push(warning(
            "stderr_present",
            "stderr was preserved in the compacted output",
        ));
    }

    let body = match strategy {
        CompactionStrategy::GitStatus => compact_git_status(&input, &mut metadata),
        CompactionStrategy::GitDiff => compact_git_diff(&input, &mut metadata),
        CompactionStrategy::Cargo => compact_build_like(&input, &mut metadata, "cargo"),
        CompactionStrategy::Dotnet => compact_build_like(&input, &mut metadata, "dotnet"),
        CompactionStrategy::Javascript => compact_build_like(&input, &mut metadata, "javascript"),
        CompactionStrategy::Docker => compact_docker(&input, &mut metadata),
        CompactionStrategy::Search => compact_search(&input, &mut metadata),
        CompactionStrategy::FilePreview => compact_preview(&input, &mut metadata),
        CompactionStrategy::Generic => compact_generic(&input, &mut metadata),
    };
    let (compacted_output, truncated, omitted_sections_count) = enforce_budget(body, max_bytes);
    if truncated {
        warnings.push(warning(
            "truncated",
            "compacted output exceeded the requested budget",
        ));
    }
    let key_findings = key_findings(&compacted_output, input.exit_code);
    let compacted_byte_estimate = compacted_output.len();
    CommandOutputSummary {
        compacted_output,
        key_findings,
        warnings,
        omitted_sections_count,
        original_byte_estimate,
        compacted_byte_estimate,
        estimated_token_savings: estimate_token_savings(
            original_byte_estimate,
            compacted_byte_estimate,
        ),
        command_family: family,
        strategy_used: strategy,
        truncated,
        metadata,
    }
}

pub fn detect_command_family(command: &str, argv: &[String]) -> CommandFamily {
    let first = argv
        .first()
        .map(String::as_str)
        .or_else(|| command.split_whitespace().next())
        .unwrap_or("")
        .trim_matches('"')
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    let second = argv
        .get(1)
        .map(String::as_str)
        .or_else(|| command.split_whitespace().nth(1))
        .unwrap_or("")
        .to_ascii_lowercase();

    match first.as_str() {
        "git" => CommandFamily::Git,
        "cargo" => CommandFamily::Cargo,
        "dotnet" => CommandFamily::Dotnet,
        "npm" | "npx" => CommandFamily::Npm,
        "pnpm" => CommandFamily::Pnpm,
        "yarn" => CommandFamily::Yarn,
        "ng" => CommandFamily::Ng,
        "tsc" => CommandFamily::Tsc,
        "eslint" => CommandFamily::Eslint,
        "docker" if second == "compose" => CommandFamily::DockerCompose,
        "docker-compose" => CommandFamily::DockerCompose,
        "docker" => CommandFamily::Docker,
        "rg" | "ripgrep" => CommandFamily::Rg,
        "grep" => CommandFamily::Grep,
        "cat" | "type" | "get-content" => CommandFamily::Cat,
        "tree" => CommandFamily::Tree,
        _ => CommandFamily::Unknown,
    }
}

fn compact_git_status(input: &CommandOutputInput, metadata: &mut CompactionMetadata) -> String {
    let lines = combined_lines(input);
    let branch = lines
        .iter()
        .find(|line| line.starts_with("On branch ") || line.starts_with("## "))
        .cloned()
        .unwrap_or_else(|| "branch: unknown".to_string());
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut conflicts = Vec::new();
    let mut section = "";
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.contains("Changes to be committed") {
            section = "staged";
        } else if trimmed.contains("Changes not staged") {
            section = "unstaged";
        } else if trimmed.contains("Untracked files") {
            section = "untracked";
        } else if trimmed.contains("Unmerged paths") || trimmed.contains("both modified") {
            section = "conflicts";
        } else if is_fileish(trimmed) {
            match section {
                "staged" => staged.push(trimmed.to_string()),
                "unstaged" => unstaged.push(trimmed.to_string()),
                "untracked" => untracked.push(trimmed.to_string()),
                "conflicts" => conflicts.push(trimmed.to_string()),
                _ => {}
            }
        }
    }
    metadata
        .fields
        .insert("staged_count".to_string(), staged.len().to_string());
    metadata
        .fields
        .insert("unstaged_count".to_string(), unstaged.len().to_string());
    metadata
        .fields
        .insert("untracked_count".to_string(), untracked.len().to_string());
    metadata
        .fields
        .insert("conflict_count".to_string(), conflicts.len().to_string());
    format!(
        "git status summary\n{branch}\nstaged: {}\n{}\nunstaged: {}\n{}\nuntracked: {}\n{}\nconflicts: {}\n{}",
        staged.len(),
        capped_list(&staged, MAX_FILES),
        unstaged.len(),
        capped_list(&unstaged, MAX_FILES),
        untracked.len(),
        capped_list(&untracked, MAX_FILES),
        conflicts.len(),
        capped_list(&conflicts, MAX_FILES)
    )
}

fn compact_git_diff(input: &CommandOutputInput, metadata: &mut CompactionMetadata) -> String {
    let lines = combined_lines(input);
    let mut files = BTreeSet::new();
    let mut headers = Vec::new();
    let mut snippets = Vec::new();
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut binary = 0usize;
    for line in &lines {
        if let Some(path) = line.strip_prefix("diff --git ") {
            files.insert(path.to_string());
        } else if line.starts_with("+++ ") || line.starts_with("--- ") {
            files.insert(line.to_string());
        } else if line.starts_with("@@") {
            headers.push(line.clone());
        } else if line.starts_with("Binary files") {
            binary += 1;
        } else if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
            push_capped(&mut snippets, line.clone(), MAX_SNIPPETS);
        } else if line.starts_with('-') && !line.starts_with("---") {
            deleted += 1;
            push_capped(&mut snippets, line.clone(), MAX_SNIPPETS);
        }
    }
    metadata
        .fields
        .insert("files".to_string(), files.len().to_string());
    metadata
        .fields
        .insert("added".to_string(), added.to_string());
    metadata
        .fields
        .insert("deleted".to_string(), deleted.to_string());
    format!(
        "git diff summary\nfiles_changed: {}\nadded_lines: {added}\ndeleted_lines: {deleted}\nbinary_files: {binary}\nhunks: {}\nfiles:\n{}\nimportant snippets:\n{}",
        files.len(),
        headers.len(),
        capped_list(&files.into_iter().collect::<Vec<_>>(), MAX_FILES),
        capped_list(&snippets, MAX_SNIPPETS)
    )
}

fn compact_build_like(
    input: &CommandOutputInput,
    metadata: &mut CompactionMetadata,
    label: &str,
) -> String {
    let lines = combined_lines(input);
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let mut summaries = Vec::new();
    for line in &lines {
        let lower = line.to_ascii_lowercase();
        if lower.contains("error")
            || lower.contains("failed")
            || lower.contains("exception")
            || lower.contains("panic")
        {
            push_capped(&mut errors, line.clone(), MAX_SNIPPETS);
        } else if lower.contains("warning") {
            push_capped(&mut warnings, line.clone(), MAX_SNIPPETS);
        }
        if lower.contains("test") && (lower.contains("failed") || lower.contains("failures")) {
            push_capped(&mut failures, line.clone(), MAX_SNIPPETS);
        }
        if lower.contains("test result:")
            || lower.contains("build failed")
            || lower.contains("build succeeded")
            || lower.contains("compilation complete")
            || lower.contains("found ")
        {
            push_capped(&mut summaries, line.clone(), MAX_SNIPPETS);
        }
    }
    metadata
        .fields
        .insert("error_count".to_string(), errors.len().to_string());
    metadata
        .fields
        .insert("warning_count".to_string(), warnings.len().to_string());
    format!(
        "{label} summary\nstatus: {}\nerrors:\n{}\nwarnings: {}\n{}\nfailed tests / exceptions:\n{}\nfinal summary:\n{}",
        status(input),
        capped_list(&errors, MAX_SNIPPETS),
        warnings.len(),
        capped_list(&warnings, MAX_SNIPPETS),
        capped_list(&failures, MAX_SNIPPETS),
        capped_list(&summaries, MAX_SNIPPETS)
    )
}

fn compact_docker(input: &CommandOutputInput, metadata: &mut CompactionMetadata) -> String {
    let lines = combined_lines(input);
    let mut services = Vec::new();
    let mut errors = Vec::new();
    for line in &lines {
        let lower = line.to_ascii_lowercase();
        if lower.contains("unhealthy")
            || lower.contains("exited")
            || lower.contains("error")
            || lower.contains("failed")
        {
            push_capped(&mut errors, line.clone(), MAX_SNIPPETS);
        }
        if lower.contains("container")
            || lower.contains("service")
            || lower.contains("0.0.0.0:")
            || lower.contains("127.0.0.1:")
        {
            push_capped(&mut services, line.clone(), MAX_FILES);
        }
    }
    metadata
        .fields
        .insert("service_lines".to_string(), services.len().to_string());
    format!(
        "docker summary\nstatus: {}\nservices/ports:\n{}\nissues:\n{}",
        status(input),
        capped_list(&services, MAX_FILES),
        capped_list(&errors, MAX_SNIPPETS)
    )
}

fn compact_search(input: &CommandOutputInput, metadata: &mut CompactionMetadata) -> String {
    let lines = combined_lines(input);
    let mut files = BTreeSet::new();
    let mut matches = Vec::new();
    for line in &lines {
        if let Some((file, _rest)) = line.split_once(':') {
            if !file.trim().is_empty() {
                files.insert(file.to_string());
            }
        }
        push_capped(&mut matches, line.clone(), MAX_SNIPPETS);
    }
    metadata
        .fields
        .insert("match_count".to_string(), lines.len().to_string());
    metadata
        .fields
        .insert("file_count".to_string(), files.len().to_string());
    format!(
        "search summary\nmatches: {}\nfiles: {}\ntop files:\n{}\nfirst matches:\n{}",
        lines.len(),
        files.len(),
        capped_list(&files.into_iter().collect::<Vec<_>>(), MAX_FILES),
        capped_list(&matches, MAX_SNIPPETS)
    )
}

fn compact_preview(input: &CommandOutputInput, metadata: &mut CompactionMetadata) -> String {
    let lines = combined_lines(input);
    let dirs = lines
        .iter()
        .filter(|line| line.ends_with('\\') || line.ends_with('/'))
        .count();
    metadata
        .fields
        .insert("preview_lines".to_string(), lines.len().to_string());
    format!(
        "preview summary\nlines: {}\ndirectory_like_lines: {dirs}\nfirst lines:\n{}",
        lines.len(),
        capped_list(&lines, MAX_SNIPPETS)
    )
}

fn compact_generic(input: &CommandOutputInput, metadata: &mut CompactionMetadata) -> String {
    let lines = combined_lines(input);
    metadata
        .fields
        .insert("generic_lines".to_string(), lines.len().to_string());
    format!(
        "command summary\nstatus: {}\nexit_code: {:?}\nimportant output:\n{}",
        status(input),
        input.exit_code,
        capped_list(&important_lines(&lines), MAX_SNIPPETS)
    )
}

fn combined_lines(input: &CommandOutputInput) -> Vec<String> {
    input
        .stderr
        .lines()
        .chain(input.stdout.lines())
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn important_lines(lines: &[String]) -> Vec<String> {
    let important = lines
        .iter()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error")
                || lower.contains("failed")
                || lower.contains("warning")
                || lower.contains("panic")
                || lower.contains("exception")
        })
        .cloned()
        .collect::<Vec<_>>();
    if important.is_empty() {
        lines.iter().take(MAX_SNIPPETS).cloned().collect()
    } else {
        important
    }
}

fn enforce_budget(output: String, max_bytes: usize) -> (String, bool, usize) {
    if output.len() <= max_bytes {
        return (output, false, 0);
    }
    let mut compacted = String::new();
    let mut kept = 0usize;
    let total = output.lines().count();
    for line in output.lines() {
        if compacted.len() + line.len() + 1 > max_bytes.saturating_sub(80) {
            break;
        }
        compacted.push_str(line);
        compacted.push('\n');
        kept += 1;
    }
    let omitted = total.saturating_sub(kept);
    compacted.push_str(&format!("[truncated: omitted {omitted} lines]\n"));
    (compacted, true, omitted)
}

fn key_findings(output: &str, exit_code: Option<i32>) -> Vec<String> {
    let mut findings = Vec::new();
    if exit_code.unwrap_or(0) != 0 {
        findings.push(format!(
            "non-zero exit code: {}",
            exit_code.unwrap_or_default()
        ));
    }
    for line in output.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("error")
            || lower.contains("failed")
            || lower.contains("warning")
            || lower.contains("conflict")
        {
            push_capped(&mut findings, line.to_string(), MAX_FINDINGS);
        }
    }
    findings
}

fn estimate_token_savings(original_bytes: usize, compacted_bytes: usize) -> usize {
    original_bytes.saturating_sub(compacted_bytes) / 4
}

fn status(input: &CommandOutputInput) -> &'static str {
    if input.exit_code.unwrap_or(0) == 0 {
        "passed"
    } else {
        "failed"
    }
}

fn capped_list(items: &[String], limit: usize) -> String {
    if items.is_empty() {
        return "- none".to_string();
    }
    let mut output = items
        .iter()
        .take(limit)
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");
    if items.len() > limit {
        output.push_str(&format!("\n- ... omitted {} more", items.len() - limit));
    }
    output
}

fn push_capped(items: &mut Vec<String>, item: String, limit: usize) {
    if items.len() < limit {
        items.push(item);
    }
}

fn is_fileish(line: &str) -> bool {
    line.contains('/')
        || line.contains('\\')
        || line.contains(".rs")
        || line.contains(".ts")
        || line.contains(".js")
        || line.contains(".md")
        || line.starts_with("modified:")
        || line.starts_with("new file:")
        || line.starts_with("deleted:")
}

fn warning(code: &str, message: &str) -> CompactionWarning {
    CompactionWarning {
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(command: &str, stdout: &str, stderr: &str, exit_code: i32) -> CommandOutputInput {
        CommandOutputInput {
            command: command.to_string(),
            argv: Vec::new(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code: Some(exit_code),
            working_directory: None,
            max_bytes: Some(1_200),
        }
    }

    #[test]
    fn detects_command_families() {
        assert_eq!(detect_command_family("git status", &[]), CommandFamily::Git);
        assert_eq!(
            detect_command_family("cargo test", &[]),
            CommandFamily::Cargo
        );
        assert_eq!(
            detect_command_family("dotnet build", &[]),
            CommandFamily::Dotnet
        );
        assert_eq!(detect_command_family("npm test", &[]), CommandFamily::Npm);
        assert_eq!(detect_command_family("pnpm test", &[]), CommandFamily::Pnpm);
        assert_eq!(detect_command_family("yarn test", &[]), CommandFamily::Yarn);
        assert_eq!(detect_command_family("ng test", &[]), CommandFamily::Ng);
        assert_eq!(
            detect_command_family("tsc --noEmit", &[]),
            CommandFamily::Tsc
        );
        assert_eq!(
            detect_command_family("eslint .", &[]),
            CommandFamily::Eslint
        );
        assert_eq!(
            detect_command_family("docker compose ps", &[]),
            CommandFamily::DockerCompose
        );
        assert_eq!(detect_command_family("rg run", &[]), CommandFamily::Rg);
        assert_eq!(
            detect_command_family("grep run file", &[]),
            CommandFamily::Grep
        );
        assert_eq!(
            detect_command_family("cat README.md", &[]),
            CommandFamily::Cat
        );
        assert_eq!(detect_command_family("tree", &[]), CommandFamily::Tree);
        assert_eq!(detect_command_family("wat", &[]), CommandFamily::Unknown);
    }

    #[test]
    fn compacts_git_status() {
        let result = compact_command_output(input(
            "git status",
            "On branch main\nChanges to be committed:\n  modified: src/lib.rs\nUntracked files:\n  docs/notes.txt\n",
            "",
            0,
        ));
        assert_eq!(result.strategy_used, CompactionStrategy::GitStatus);
        assert!(result.compacted_output.contains("staged: 1"));
        assert!(result.compacted_output.contains("untracked: 1"));
    }

    #[test]
    fn compacts_git_diff() {
        let result = compact_command_output(input(
            "git diff",
            "diff --git a/src/lib.rs b/src/lib.rs\n@@ fn run @@\n-old\n+new\n",
            "",
            0,
        ));
        assert!(result.compacted_output.contains("added_lines: 1"));
        assert!(result.compacted_output.contains("deleted_lines: 1"));
    }

    #[test]
    fn compacts_cargo_output_and_preserves_failure() {
        let result = compact_command_output(input(
            "cargo test",
            "error[E0425]: cannot find value\nwarning: unused variable\ntest result: FAILED. 1 failed\n",
            "thread 'x' panicked\n",
            101,
        ));
        assert_eq!(result.command_family, CommandFamily::Cargo);
        assert!(result.compacted_output.contains("status: failed"));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.code == "non_zero_exit"));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.code == "stderr_present"));
    }

    #[test]
    fn compacts_dotnet_javascript_docker_and_search_outputs() {
        assert_eq!(
            compact_command_output(input("dotnet test", "XUnit failed: MyTest\n", "", 1))
                .strategy_used,
            CompactionStrategy::Dotnet
        );
        assert_eq!(
            compact_command_output(input("npm test", "TS2322 error\n", "", 1)).strategy_used,
            CompactionStrategy::Javascript
        );
        assert_eq!(
            compact_command_output(input(
                "tsc --noEmit",
                "src/app.ts(1,1): error TS2322\n",
                "",
                1
            ))
            .strategy_used,
            CompactionStrategy::Javascript
        );
        assert_eq!(
            compact_command_output(input(
                "eslint .",
                "src/app.ts  1:1  error  no-undef\n",
                "",
                1
            ))
            .strategy_used,
            CompactionStrategy::Javascript
        );
        assert_eq!(
            compact_command_output(input(
                "docker compose ps",
                "api unhealthy 127.0.0.1:8080\n",
                "",
                1
            ))
            .strategy_used,
            CompactionStrategy::Docker
        );
        let search = compact_command_output(input("rg run", "src/lib.rs:10:run\n", "", 0));
        assert_eq!(search.strategy_used, CompactionStrategy::Search);
        assert!(search.compacted_output.contains("matches: 1"));
        assert_eq!(
            compact_command_output(input("grep run src/lib.rs", "src/lib.rs:run\n", "", 0))
                .strategy_used,
            CompactionStrategy::Search
        );
    }

    #[test]
    fn compacts_cat_and_tree_previews() {
        let cat = compact_command_output(input("cat README.md", "# Title\nbody\n", "", 0));
        assert_eq!(cat.strategy_used, CompactionStrategy::FilePreview);
        assert!(cat.compacted_output.contains("first lines"));
        let tree = compact_command_output(input("tree", "src/\nsrc/lib.rs\n", "", 0));
        assert_eq!(tree.strategy_used, CompactionStrategy::FilePreview);
    }

    #[test]
    fn generic_compaction_truncates_deterministically() {
        let long_output = (0..200)
            .map(|index| format!("error line {index}: repeated failure detail"))
            .collect::<Vec<_>>()
            .join("\n");
        let result = compact_command_output(CommandOutputInput {
            command: "unknown".to_string(),
            argv: Vec::new(),
            stdout: long_output,
            stderr: "fatal error".to_string(),
            exit_code: Some(2),
            working_directory: None,
            max_bytes: Some(400),
        });
        assert!(result.truncated);
        assert!(result.estimated_token_savings > 0);
        assert!(result.compacted_output.contains("fatal error"));
    }
}
