use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use b3_core::{
    BranchId, ContractError, ContractResult, EditApplyResult, EditMode, EditOperation, EditPlan,
    EditPreview, EditRequest, EditSafetyError, EditTargetKind, EditValidationWarning, FileEdit,
    ProjectId, QueryRepository, QueryScope, QuerySymbol, SymbolId, TextEdit,
};

const DEFAULT_PROJECT_ID: &str = "default";
const DEFAULT_BRANCH: &str = "main";
const DEFAULT_MAX_CHANGED_FILES: usize = 1;
const DEFAULT_MAX_CHANGED_BYTES: usize = 64 * 1024;
const MAX_PATCH_CHARS: usize = 12_000;

pub struct SymbolicEditEngine<R> {
    repository: R,
}

impl<R> SymbolicEditEngine<R>
where
    R: QueryRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn preview_edit(&self, request: EditRequest) -> ContractResult<EditPlan> {
        self.plan_edit(request, true)
    }

    pub fn apply_edit(&self, request: EditRequest) -> ContractResult<EditApplyResult> {
        let dry_run = request.dry_run.unwrap_or(true);
        let mode = request.mode.clone().unwrap_or_default();
        if dry_run || mode != EditMode::Apply {
            return Err(safety_error(
                "explicit_apply_required",
                "apply requires mode=apply and dry_run=false",
            ));
        }
        let plan = self.plan_edit(request, false)?;
        if !plan.safety_errors.is_empty() {
            return Err(safety_error(
                "unsafe_plan",
                format!("edit plan has {} safety errors", plan.safety_errors.len()),
            ));
        }

        let mut backup_paths = Vec::new();
        let mut changed_files = Vec::new();
        for edit in &plan.file_edits {
            let project_root = PathBuf::from(&plan.project_path);
            let file_path = resolve_under_root(&project_root, &edit.file_path)?;
            let current = read_text_file(&file_path)?;
            if stable_hash(&current) != edit.old_content_hash {
                return Err(safety_error(
                    "stale_file",
                    "file changed after preview/planning; re-run preview before applying",
                ));
            }
            let new_content = apply_text_edit(&current, &edit.text_edit)?;
            if plan.create_backup {
                let backup = create_backup(&project_root, &file_path, &edit.file_path, &current)?;
                backup_paths.push(backup.display().to_string());
            }
            atomic_write(&file_path, &new_content)?;
            changed_files.push(edit.file_path.clone());
        }

        let mut warnings = plan.warnings.clone();
        warnings.push(warning(
            "reindex_recommended",
            "Reindex recommended after apply; Phase 12 does not update the index automatically",
        ));
        Ok(EditApplyResult {
            applied: true,
            dry_run: false,
            plan_id: plan.plan_id,
            changed_files,
            backup_paths,
            patch: plan.preview.patch,
            warnings,
        })
    }

    fn plan_edit(&self, request: EditRequest, force_preview: bool) -> ContractResult<EditPlan> {
        let dry_run = if force_preview {
            true
        } else {
            request.dry_run.unwrap_or(true)
        };
        let mode = if force_preview {
            EditMode::Preview
        } else {
            request.mode.clone().unwrap_or_default()
        };
        let create_backup = request.create_backup.unwrap_or(true);
        let allow_multi_file = request.allow_multi_file.unwrap_or(false);
        let max_changed_files = request
            .max_changed_files
            .unwrap_or(DEFAULT_MAX_CHANGED_FILES)
            .min(DEFAULT_MAX_CHANGED_FILES);
        let max_changed_bytes = request
            .max_changed_bytes
            .unwrap_or(DEFAULT_MAX_CHANGED_BYTES)
            .min(DEFAULT_MAX_CHANGED_BYTES);
        if allow_multi_file {
            return Err(safety_error(
                "multi_file_deferred",
                "Phase 12 supports single-file symbolic edits only",
            ));
        }

        let project_root = request
            .project_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let project_root = canonical_root(&project_root)?;
        let project_id = request
            .project_id
            .clone()
            .unwrap_or_else(|| DEFAULT_PROJECT_ID.to_string());
        let branch = request
            .branch
            .clone()
            .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
        let scope = QueryScope::new(ProjectId::new(&project_id), BranchId::new(&branch));
        let resolved = self.resolve_target(&scope, &project_root, &request)?;
        let current = read_text_file(&resolved.absolute_path)?;
        let newline = detect_newline(&current);
        let text_edit = build_text_edit(
            &current,
            newline,
            &request.operation,
            &resolved,
            &request.new_text,
        )?;
        if let Some(expected) = &request.expected_current_text {
            if &text_edit.old_text != expected {
                return Err(safety_error(
                    "stale_expected_text",
                    "expected_current_text did not match the resolved target text",
                ));
            }
        }
        let new_content = apply_text_edit(&current, &text_edit)?;
        let changed_bytes = byte_delta(&text_edit.old_text, &text_edit.new_text);
        if changed_bytes > max_changed_bytes {
            return Err(safety_error(
                "edit_too_large",
                format!(
                    "edit changes {changed_bytes} bytes, above max_changed_bytes={max_changed_bytes}"
                ),
            ));
        }
        let changed_lines = changed_line_count(&text_edit.old_text, &text_edit.new_text);
        let relative_path = resolved.relative_path.clone();
        let file_edit = FileEdit {
            file_path: relative_path.clone(),
            operation: request.operation.clone(),
            old_content_hash: stable_hash(&current),
            new_content_hash: stable_hash(&new_content),
            changed_bytes,
            changed_lines,
            text_edit,
        };
        let patch = unified_diff(
            &relative_path,
            &current,
            &new_content,
            file_edit.text_edit.start_line,
        );
        let (patch, mut warnings) = bound_patch(patch);
        if max_changed_files != 1 {
            warnings.push(warning(
                "changed_file_bound",
                "Phase 12 clamps max_changed_files to 1",
            ));
        }
        let preview = EditPreview {
            file_path: relative_path,
            operation: request.operation.clone(),
            target_summary: resolved.summary,
            old_snippet: compact_snippet(&file_edit.text_edit.old_text),
            new_snippet: compact_snippet(&file_edit.text_edit.new_text),
            changed_line_count: file_edit.changed_lines,
            changed_byte_count: file_edit.changed_bytes,
            safety_status: "safe_to_apply_with_explicit_apply".to_string(),
            warnings: warnings.clone(),
            patch,
        };
        let plan_id = plan_id(&project_root, &file_edit);
        Ok(EditPlan {
            plan_id,
            mode,
            dry_run,
            create_backup,
            project_path: project_root.display().to_string(),
            database_path: request.database_path.clone(),
            project_id,
            branch,
            target: request.target,
            operation: request.operation,
            file_edits: vec![file_edit],
            preview,
            warnings,
            safety_errors: Vec::new(),
        })
    }

    fn resolve_target(
        &self,
        scope: &QueryScope,
        project_root: &Path,
        request: &EditRequest,
    ) -> ContractResult<ResolvedTarget> {
        match request.target.kind {
            EditTargetKind::FileRange => {
                let relative_path = required_file_path(request)?;
                let absolute_path = resolve_under_root(project_root, &relative_path)?;
                let range = ResolvedRange::from_request(request)?;
                Ok(ResolvedTarget {
                    absolute_path,
                    relative_path,
                    range,
                    summary: "explicit file range".to_string(),
                })
            }
            EditTargetKind::File => {
                let relative_path = required_file_path(request)?;
                let absolute_path = resolve_under_root(project_root, &relative_path)?;
                Ok(ResolvedTarget {
                    absolute_path,
                    relative_path,
                    range: ResolvedRange::whole_file(),
                    summary: "whole file".to_string(),
                })
            }
            EditTargetKind::Symbol => self.resolve_symbol(scope, project_root, request),
            EditTargetKind::QueryResult => Err(safety_error(
                "query_result_edit_deferred",
                "query-result edit targets are deferred until they can be made unambiguous",
            )),
        }
    }

    fn resolve_symbol(
        &self,
        scope: &QueryScope,
        project_root: &Path,
        request: &EditRequest,
    ) -> ContractResult<ResolvedTarget> {
        let mut candidates = if let Some(symbol_id) = &request.target.symbol_id {
            self.repository
                .get_symbol(scope, &SymbolId::new(symbol_id))?
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            let name = request.target.symbol_name.as_deref().ok_or_else(|| {
                safety_error("missing_symbol_name", "symbol target requires symbol_name")
            })?;
            self.repository.find_symbols(scope, name)?
        };
        if let Some(file_path) = &request.target.file_path {
            candidates = candidates
                .into_iter()
                .filter(|symbol| {
                    self.repository
                        .get_file(scope, &symbol.file_id)
                        .ok()
                        .flatten()
                        .is_some_and(|file| {
                            normalize_slashes(&file.path) == normalize_slashes(file_path)
                        })
                })
                .collect();
        }
        match candidates.len() {
            0 => Err(safety_error(
                "symbol_not_found",
                "no matching indexed symbol found",
            )),
            1 => {
                let symbol = candidates.remove(0);
                let file = self
                    .repository
                    .get_file(scope, &symbol.file_id)?
                    .ok_or_else(|| {
                        safety_error("file_not_found", "indexed symbol file is missing")
                    })?;
                let absolute_path = resolve_under_root(project_root, &file.path)?;
                Ok(ResolvedTarget {
                    absolute_path,
                    relative_path: file.path,
                    range: ResolvedRange {
                        start_line: symbol.start_line.max(1),
                        start_column: 1,
                        end_line: symbol.end_line.max(symbol.start_line).max(1),
                        end_column: None,
                    },
                    summary: format!("symbol {} ({})", symbol.name, symbol.id.as_str()),
                })
            }
            _ => Err(safety_error(
                "ambiguous_symbol",
                format!(
                    "symbol target matched multiple symbols: {}",
                    candidates
                        .iter()
                        .map(symbol_candidate)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedTarget {
    absolute_path: PathBuf,
    relative_path: String,
    range: ResolvedRange,
    summary: String,
}

#[derive(Debug, Clone)]
struct ResolvedRange {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: Option<usize>,
}

impl ResolvedRange {
    fn from_request(request: &EditRequest) -> ContractResult<Self> {
        let start_line = request
            .target
            .start_line
            .ok_or_else(|| safety_error("missing_range", "file_range requires start_line"))?;
        let end_line = request
            .target
            .end_line
            .ok_or_else(|| safety_error("missing_range", "file_range requires end_line"))?;
        let start_column = request.target.start_column.unwrap_or(1);
        let end_column = request.target.end_column;
        if start_line == 0 || end_line == 0 || start_column == 0 || end_column == Some(0) {
            return Err(safety_error(
                "invalid_range",
                "line and column values are 1-based",
            ));
        }
        if start_line > end_line
            || (start_line == end_line && end_column.is_some_and(|end| start_column > end))
        {
            return Err(safety_error(
                "invalid_range",
                "range start must be before range end",
            ));
        }
        Ok(Self {
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    fn whole_file() -> Self {
        Self {
            start_line: 1,
            start_column: 1,
            end_line: usize::MAX,
            end_column: None,
        }
    }
}

fn build_text_edit(
    content: &str,
    newline: &str,
    operation: &EditOperation,
    target: &ResolvedTarget,
    requested_new_text: &str,
) -> ContractResult<TextEdit> {
    let (start, end, start_line, start_column, end_line, end_column) = match operation {
        EditOperation::AppendToFile => {
            let line = if content.ends_with('\n') {
                line_count(content) + 1
            } else {
                line_count(content).max(1)
            };
            (
                content.len(),
                content.len(),
                line,
                line_end_column(content, line),
                line,
                line_end_column(content, line),
            )
        }
        EditOperation::PrependToFile => (0, 0, 1, 1, 1, 1),
        EditOperation::InsertBeforeSymbol => {
            let start = offset_for_position(content, target.range.start_line, 1)?;
            (
                start,
                start,
                target.range.start_line,
                1,
                target.range.start_line,
                1,
            )
        }
        EditOperation::InsertAfterSymbol => {
            let end = range_end_offset(content, &target.range)?;
            let end_column = target
                .range
                .end_column
                .unwrap_or_else(|| line_end_column(content, target.range.end_line));
            (
                end,
                end,
                target.range.end_line,
                end_column,
                target.range.end_line,
                end_column,
            )
        }
        EditOperation::ReplaceRange | EditOperation::ReplaceSymbolBody => {
            let start =
                offset_for_position(content, target.range.start_line, target.range.start_column)?;
            let end = range_end_offset(content, &target.range)?;
            let end_column = target
                .range
                .end_column
                .unwrap_or_else(|| line_end_column(content, target.range.end_line));
            (
                start,
                end,
                target.range.start_line,
                target.range.start_column,
                target.range.end_line,
                end_column,
            )
        }
    };
    if start > end || end > content.len() {
        return Err(safety_error(
            "invalid_range",
            "resolved byte range is invalid",
        ));
    }
    let new_text = match operation {
        EditOperation::AppendToFile if !content.is_empty() && !content.ends_with('\n') => {
            format!("{newline}{requested_new_text}")
        }
        _ => requested_new_text.to_string(),
    };
    Ok(TextEdit {
        start_line,
        start_column,
        end_line,
        end_column,
        old_text: content[start..end].to_string(),
        new_text,
    })
}

fn apply_text_edit(content: &str, edit: &TextEdit) -> ContractResult<String> {
    let start = offset_for_position(content, edit.start_line, edit.start_column)?;
    let end = start + edit.old_text.len();
    if start > end || end > content.len() {
        return Err(safety_error(
            "invalid_range",
            "edit range is invalid for current content",
        ));
    }
    if content[start..end] != edit.old_text {
        return Err(safety_error(
            "stale_file",
            "target text no longer matches the planned edit",
        ));
    }
    let mut next = String::with_capacity(content.len() - (end - start) + edit.new_text.len());
    next.push_str(&content[..start]);
    next.push_str(&edit.new_text);
    next.push_str(&content[end..]);
    Ok(next)
}

fn required_file_path(request: &EditRequest) -> ContractResult<String> {
    request
        .target
        .file_path
        .clone()
        .ok_or_else(|| safety_error("missing_file_path", "edit target requires file_path"))
}

fn canonical_root(path: &Path) -> ContractResult<PathBuf> {
    path.canonicalize()
        .map_err(|error| safety_error("invalid_project_root", error.to_string()))
}

fn resolve_under_root(root: &Path, relative_or_abs: &str) -> ContractResult<PathBuf> {
    let raw = PathBuf::from(relative_or_abs);
    if relative_or_abs.contains('\0') {
        return Err(safety_error("invalid_path", "path contains NUL"));
    }
    if raw
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(safety_error(
            "path_traversal",
            "path traversal is not allowed",
        ));
    }
    let candidate = if raw.is_absolute() {
        raw
    } else {
        root.join(raw)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|error| safety_error("invalid_target_path", error.to_string()))?;
    if !canonical.starts_with(root) {
        return Err(safety_error(
            "outside_project_root",
            "target path is outside project root",
        ));
    }
    Ok(canonical)
}

fn read_text_file(path: &Path) -> ContractResult<String> {
    let bytes = fs::read(path).map_err(|error| safety_error("read_failed", error.to_string()))?;
    if bytes.contains(&0) {
        return Err(safety_error("binary_file", "binary files cannot be edited"));
    }
    String::from_utf8(bytes).map_err(|_| safety_error("invalid_utf8", "file is not valid UTF-8"))
}

fn offset_for_position(content: &str, line: usize, column: usize) -> ContractResult<usize> {
    if line == 0 || column == 0 {
        return Err(safety_error("invalid_position", "positions are 1-based"));
    }
    let starts = line_starts(content);
    if line == starts.len() + 1 && column == 1 && content.ends_with('\n') {
        return Ok(content.len());
    }
    if line > starts.len() {
        return Err(safety_error("invalid_range", "line is outside file"));
    }
    let start = starts[line - 1];
    let line_end = if line < starts.len() {
        starts[line]
    } else {
        content.len()
    };
    let line_text = &content[start..line_end].trim_end_matches(['\r', '\n']);
    if column == 1 {
        return Ok(start);
    }
    let mut byte = start;
    for (idx, ch) in line_text.chars().enumerate() {
        if idx + 1 == column - 1 {
            return Ok(byte);
        }
        byte += ch.len_utf8();
    }
    if column == line_text.chars().count() + 1 {
        Ok(start + line_text.len())
    } else {
        Err(safety_error("invalid_range", "column is outside line"))
    }
}

fn range_end_offset(content: &str, range: &ResolvedRange) -> ContractResult<usize> {
    if let Some(column) = range.end_column {
        offset_for_position(content, range.end_line, column)
    } else if range.end_line == usize::MAX {
        Ok(content.len())
    } else {
        let starts = line_starts(content);
        if range.end_line > starts.len() {
            return Err(safety_error("invalid_range", "end_line is outside file"));
        }
        if range.end_line < starts.len() {
            Ok(starts[range.end_line])
        } else {
            Ok(content.len())
        }
    }
}

fn line_starts(content: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, byte) in content.bytes().enumerate() {
        if byte == b'\n' && idx + 1 < content.len() {
            starts.push(idx + 1);
        }
    }
    starts
}

fn line_count(content: &str) -> usize {
    line_starts(content).len()
}

fn line_end_column(content: &str, line: usize) -> usize {
    let starts = line_starts(content);
    if line == 0 || line > starts.len() {
        return 1;
    }
    let start = starts[line - 1];
    let end = if line < starts.len() {
        starts[line]
    } else {
        content.len()
    };
    content[start..end]
        .trim_end_matches(['\r', '\n'])
        .chars()
        .count()
        + 1
}

fn detect_newline(content: &str) -> &str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn plan_id(root: &Path, edit: &FileEdit) -> String {
    stable_hash(&format!(
        "{}:{}:{}:{}",
        root.display(),
        edit.file_path,
        edit.old_content_hash,
        edit.new_content_hash
    ))
}

fn byte_delta(old: &str, new: &str) -> usize {
    old.len().abs_diff(new.len()) + old.len().min(new.len())
}

fn changed_line_count(old: &str, new: &str) -> usize {
    old.lines().count().max(new.lines().count()).max(1)
}

fn compact_snippet(value: &str) -> String {
    let mut snippet = value.chars().take(800).collect::<String>();
    if value.chars().count() > 800 {
        snippet.push_str("\n... truncated ...");
    }
    snippet
}

fn unified_diff(path: &str, old: &str, new: &str, start_line: usize) -> String {
    let old_lines = old.lines().collect::<Vec<_>>();
    let new_lines = new.lines().collect::<Vec<_>>();
    let first = old_lines
        .iter()
        .zip(new_lines.iter())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| old_lines.len().min(new_lines.len()));
    let old_tail = old_lines.len().saturating_sub(first);
    let new_tail = new_lines.len().saturating_sub(first);
    let context_start = first.saturating_sub(2);
    let old_end = (first + old_tail + 2).min(old_lines.len());
    let new_end = (first + new_tail + 2).min(new_lines.len());
    let mut diff = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -{},{} +{},{} @@\n",
        start_line + context_start,
        old_end.saturating_sub(context_start),
        start_line + context_start,
        new_end.saturating_sub(context_start)
    );
    for line in &old_lines[context_start..first.min(old_lines.len())] {
        diff.push(' ');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in &old_lines[first.min(old_lines.len())..old_end] {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in &new_lines[first.min(new_lines.len())..new_end] {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

fn bound_patch(mut patch: String) -> (String, Vec<EditValidationWarning>) {
    if patch.len() <= MAX_PATCH_CHARS {
        return (patch, Vec::new());
    }
    patch.truncate(MAX_PATCH_CHARS);
    patch.push_str("\n... patch truncated ...\n");
    (
        patch,
        vec![warning(
            "patch_truncated",
            "patch exceeded maximum preview size and was truncated",
        )],
    )
}

fn create_backup(
    root: &Path,
    file: &Path,
    relative_path: &str,
    content: &str,
) -> ContractResult<PathBuf> {
    let backup_root = root.join(".b3").join("backups").join("edits");
    fs::create_dir_all(&backup_root)
        .map_err(|error| safety_error("backup_failed", error.to_string()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let safe_name = normalize_slashes(relative_path).replace('/', "__");
    let backup = backup_root.join(format!("{stamp}-{safe_name}.bak"));
    if !file.starts_with(root) {
        return Err(safety_error(
            "outside_project_root",
            "backup target outside root",
        ));
    }
    fs::write(&backup, content)
        .map_err(|error| safety_error("backup_failed", error.to_string()))?;
    Ok(backup)
}

fn atomic_write(path: &Path, content: &str) -> ContractResult<()> {
    let tmp = path.with_extension("b3-edit-tmp");
    {
        let mut file = fs::File::create(&tmp)
            .map_err(|error| safety_error("write_failed", error.to_string()))?;
        file.write_all(content.as_bytes())
            .map_err(|error| safety_error("write_failed", error.to_string()))?;
        file.sync_all()
            .map_err(|error| safety_error("write_failed", error.to_string()))?;
    }
    fs::rename(&tmp, path).map_err(|error| safety_error("write_failed", error.to_string()))
}

fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn symbol_candidate(symbol: &QuerySymbol) -> String {
    format!(
        "{}:{}:{}",
        symbol.id.as_str(),
        symbol.name,
        symbol.start_line
    )
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> EditValidationWarning {
    EditValidationWarning {
        code: code.into(),
        message: message.into(),
    }
}

fn safety_error(code: impl Into<String>, message: impl Into<String>) -> ContractError {
    let error = EditSafetyError {
        code: code.into(),
        message: message.into(),
    };
    ContractError::new(format!("{}: {}", error.code, error.message))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use b3_core::{BranchId, IndexJob, Indexer, ProjectId};
    use b3_indexer::{IndexerConfig, LocalIndexer, RustLanguagePack};
    use b3_storage::SqliteStorage;
    use tempfile::tempdir;

    use super::*;

    #[derive(Clone, Copy)]
    struct TestEventBus;

    impl b3_core::EventBus for TestEventBus {
        fn publish(&self, _event: b3_core::DomainEvent) -> ContractResult<()> {
            Ok(())
        }
    }

    struct Fixture {
        root: PathBuf,
        storage: SqliteStorage,
    }

    fn fixture(source: &str) -> (tempfile::TempDir, Fixture) {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(root.join("src").join("lib.rs"), source).expect("source");
        fs::write(root.join("src").join("text.txt"), "alpha\nbeta\n").expect("text");
        fs::write(
            root.join("src").join("crlf.rs"),
            "fn a() {\r\n    1\r\n}\r\n",
        )
        .expect("crlf");
        fs::write(root.join("src").join("binary.bin"), [0, 159, 146, 150]).expect("binary");
        fs::write(root.join("src").join("invalid.txt"), [0xff, b'a']).expect("invalid utf8");
        let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("storage");
        LocalIndexer::new(
            RustLanguagePack,
            &storage,
            TestEventBus,
            IndexerConfig {
                branch_id: BranchId::new("main"),
                ..IndexerConfig::default()
            },
        )
        .index(IndexJob {
            project_id: ProjectId::new("default"),
            root_path: root.display().to_string(),
        })
        .expect("index");
        (dir, Fixture { root, storage })
    }

    fn range_request(root: &Path) -> EditRequest {
        EditRequest {
            project_path: Some(root.display().to_string()),
            database_path: None,
            project_id: Some("default".to_string()),
            branch: Some("main".to_string()),
            target: b3_core::EditTarget {
                kind: EditTargetKind::FileRange,
                file_path: Some("src/text.txt".to_string()),
                symbol_name: None,
                symbol_id: None,
                query: None,
                start_line: Some(2),
                start_column: Some(1),
                end_line: Some(2),
                end_column: None,
            },
            operation: EditOperation::ReplaceRange,
            mode: Some(EditMode::Preview),
            dry_run: None,
            create_backup: None,
            allow_multi_file: None,
            max_changed_files: None,
            max_changed_bytes: None,
            expected_current_text: None,
            new_text: "gamma\n".to_string(),
        }
    }

    #[test]
    fn preview_replaces_explicit_range_without_writing() {
        let (_dir, fx) = fixture("pub fn first() {}\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        let plan = engine.preview_edit(range_request(&fx.root)).expect("plan");
        assert!(plan.dry_run);
        assert!(plan.preview.patch.contains("-beta"));
        assert!(plan.preview.patch.contains("+gamma"));
        assert_eq!(
            fs::read_to_string(fx.root.join("src/text.txt")).unwrap(),
            "alpha\nbeta\n"
        );
    }

    #[test]
    fn apply_requires_explicit_mode_and_creates_backup() {
        let (_dir, fx) = fixture("pub fn first() {}\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        assert!(engine.apply_edit(range_request(&fx.root)).is_err());
        let mut request = range_request(&fx.root);
        request.mode = Some(EditMode::Apply);
        request.dry_run = Some(false);
        let result = engine.apply_edit(request).expect("apply");
        assert!(result.applied);
        assert_eq!(result.backup_paths.len(), 1);
        assert_eq!(
            fs::read_to_string(fx.root.join("src/text.txt")).unwrap(),
            "alpha\ngamma\n"
        );
    }

    #[test]
    fn symbol_insertions_and_replace_use_indexed_ranges() {
        let (_dir, fx) = fixture("pub fn first() {}\npub fn second() {}\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        let base = |op, text| EditRequest {
            project_path: Some(fx.root.display().to_string()),
            database_path: None,
            project_id: Some("default".to_string()),
            branch: Some("main".to_string()),
            target: b3_core::EditTarget {
                kind: EditTargetKind::Symbol,
                file_path: Some("src/lib.rs".to_string()),
                symbol_name: Some("first".to_string()),
                symbol_id: None,
                query: None,
                start_line: None,
                start_column: None,
                end_line: None,
                end_column: None,
            },
            operation: op,
            mode: Some(EditMode::Preview),
            dry_run: Some(true),
            create_backup: None,
            allow_multi_file: None,
            max_changed_files: None,
            max_changed_bytes: None,
            expected_current_text: None,
            new_text: text,
        };
        let replace = engine
            .preview_edit(base(
                EditOperation::ReplaceSymbolBody,
                "pub fn first() { 1 }\n".to_string(),
            ))
            .expect("replace");
        assert!(replace.preview.patch.contains("+pub fn first() { 1 }"));
        let before = engine
            .preview_edit(base(
                EditOperation::InsertBeforeSymbol,
                "// before\n".to_string(),
            ))
            .expect("before");
        assert!(before.preview.patch.contains("+// before"));
        let after = engine
            .preview_edit(base(
                EditOperation::InsertAfterSymbol,
                "// after\n".to_string(),
            ))
            .expect("after");
        assert!(after.file_edits[0].text_edit.new_text.contains("// after"));
    }

    #[test]
    fn append_prepend_and_crlf_are_supported() {
        let (_dir, fx) = fixture("pub fn first() {}\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        let mut request = range_request(&fx.root);
        request.target.kind = EditTargetKind::File;
        request.target.file_path = Some("src/crlf.rs".to_string());
        request.operation = EditOperation::PrependToFile;
        request.new_text = "// top\r\n".to_string();
        let plan = engine.preview_edit(request).expect("prepend");
        assert!(plan.preview.patch.contains("+// top"));

        let mut append = range_request(&fx.root);
        append.target.kind = EditTargetKind::File;
        append.target.file_path = Some("src/crlf.rs".to_string());
        append.operation = EditOperation::AppendToFile;
        append.new_text = "// tail\r\n".to_string();
        let plan = engine.preview_edit(append).expect("append");
        assert!(plan.preview.patch.contains("+// tail"));
    }

    #[test]
    fn safety_rejects_path_traversal_binary_invalid_range_and_stale_text() {
        let (_dir, fx) = fixture("pub fn first() {}\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        let mut traversal = range_request(&fx.root);
        traversal.target.file_path = Some("../outside.rs".to_string());
        assert!(engine.preview_edit(traversal).is_err());

        let mut binary = range_request(&fx.root);
        binary.target.file_path = Some("src/binary.bin".to_string());
        assert!(engine.preview_edit(binary).is_err());

        let mut invalid_utf8 = range_request(&fx.root);
        invalid_utf8.target.file_path = Some("src/invalid.txt".to_string());
        let error = engine.preview_edit(invalid_utf8).unwrap_err();
        assert!(error.to_string().contains("invalid_utf8"));

        let outside = tempdir().expect("outside");
        let outside_file = outside.path().join("outside.rs");
        fs::write(&outside_file, "fn outside() {}\n").expect("outside");
        let mut absolute_outside = range_request(&fx.root);
        absolute_outside.target.file_path = Some(outside_file.display().to_string());
        assert!(engine.preview_edit(absolute_outside).is_err());

        let mut invalid = range_request(&fx.root);
        invalid.target.start_line = Some(3);
        invalid.target.end_line = Some(2);
        assert!(engine.preview_edit(invalid).is_err());

        let mut stale = range_request(&fx.root);
        stale.expected_current_text = Some("not beta\n".to_string());
        assert!(engine.preview_edit(stale).is_err());
    }

    #[test]
    fn ambiguous_and_missing_symbols_are_structured_errors() {
        let (_dir, fx) = fixture("pub fn dup() {}\nmod inner { pub fn dup() {} }\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        let request = EditRequest {
            project_path: Some(fx.root.display().to_string()),
            database_path: None,
            project_id: Some("default".to_string()),
            branch: Some("main".to_string()),
            target: b3_core::EditTarget {
                kind: EditTargetKind::Symbol,
                file_path: None,
                symbol_name: Some("dup".to_string()),
                symbol_id: None,
                query: None,
                start_line: None,
                start_column: None,
                end_line: None,
                end_column: None,
            },
            operation: EditOperation::ReplaceSymbolBody,
            mode: Some(EditMode::Preview),
            dry_run: Some(true),
            create_backup: None,
            allow_multi_file: None,
            max_changed_files: None,
            max_changed_bytes: None,
            expected_current_text: None,
            new_text: "pub fn dup() { 1 }\n".to_string(),
        };
        let error = engine.preview_edit(request).unwrap_err();
        assert!(error.to_string().contains("ambiguous_symbol"));
    }

    #[test]
    fn too_large_replacement_is_rejected() {
        let (_dir, fx) = fixture("pub fn first() {}\n");
        let engine = SymbolicEditEngine::new(&fx.storage);
        let mut request = range_request(&fx.root);
        request.max_changed_bytes = Some(4);
        request.new_text = "this is too large\n".to_string();
        assert!(engine.preview_edit(request).is_err());
    }
}
