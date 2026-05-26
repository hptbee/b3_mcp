use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use b3_core::{
    BranchId, ContractError, ContractResult, EdgeKind, EditMode, EditOperation,
    EditValidationWarning, FileEdit, GraphDirection, ProjectId, QueryFile, QueryRepository,
    QueryScope, QuerySymbol, RefactorKind, RefactorSafetyError, RenameApplyResult,
    RenameConfidence, RenameConflict, RenameOccurrence, RenameOccurrenceKind, RenamePlan,
    RenamePreview, RenameRequest, RenameScope, RenameTargetKind, SymbolId, TextEdit,
};

const DEFAULT_PROJECT_ID: &str = "default";
const DEFAULT_BRANCH: &str = "main";
const DEFAULT_MAX_CHANGED_FILES: usize = 1;
const DEFAULT_MAX_CHANGED_FILES_MULTI: usize = 10;
const DEFAULT_MAX_OCCURRENCES: usize = 100;
const DEFAULT_MAX_CHANGED_BYTES: usize = 128 * 1024;
const MAX_PATCH_CHARS: usize = 20_000;

pub struct RenameRefactorEngine<R> {
    repository: R,
}

impl<R> RenameRefactorEngine<R>
where
    R: QueryRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn preview_rename(&self, request: RenameRequest) -> ContractResult<RenamePlan> {
        self.plan_rename(request, true)
    }

    pub fn apply_rename(&self, request: RenameRequest) -> ContractResult<RenameApplyResult> {
        let dry_run = request.dry_run.unwrap_or(true);
        let mode = request.mode.clone().unwrap_or_default();
        if dry_run || mode != EditMode::Apply {
            return Err(refactor_error(
                "explicit_apply_required",
                "rename apply requires mode=apply and dry_run=false",
            ));
        }
        let expected_plan_id = request.expected_plan_id.clone();
        let plan = self.plan_rename(request, false)?;
        if let Some(expected) = expected_plan_id {
            if expected != plan.plan_id {
                return Err(refactor_error(
                    "stale_plan",
                    "expected_plan_id does not match the current rename plan",
                ));
            }
        }
        if !plan.conflicts.is_empty() || !plan.safety_errors.is_empty() {
            return Err(refactor_error(
                "unsafe_rename_plan",
                "rename plan has conflicts or safety errors",
            ));
        }

        let project_root = PathBuf::from(&plan.project_path);
        let grouped = group_edits_by_file(&plan.file_edits);
        let mut validated = Vec::new();
        for (file_path, edits) in &grouped {
            let absolute = resolve_under_root(&project_root, file_path)?;
            let current = read_text_file(&absolute)?;
            let expected_hash = edits
                .first()
                .map(|edit| edit.old_content_hash.as_str())
                .unwrap_or_default();
            if stable_hash(&current) != expected_hash {
                return Err(refactor_error(
                    "stale_file",
                    format!("{file_path} changed after preview; re-run rename preview"),
                ));
            }
            let next = apply_file_edits(&current, edits)?;
            validated.push((file_path.clone(), absolute, current, next));
        }

        let mut backup_paths = Vec::new();
        for (relative, absolute, current, _) in &validated {
            if plan.create_backup {
                backup_paths.push(
                    create_backup(&project_root, absolute, relative, current)?
                        .display()
                        .to_string(),
                );
            }
        }
        for (_, absolute, _, next) in &validated {
            atomic_write(absolute, next)?;
        }

        let mut warnings = plan.warnings.clone();
        warnings.push(warning(
            "reindex_recommended",
            "Reindex recommended after rename apply; Phase 13 does not update the index automatically",
        ));
        Ok(RenameApplyResult {
            applied: true,
            dry_run: false,
            plan_id: plan.plan_id,
            changed_files: validated
                .iter()
                .map(|(relative, _, _, _)| relative.clone())
                .collect(),
            backup_paths,
            patch: plan.preview.patch,
            warnings,
            reindex_recommended: true,
        })
    }

    fn plan_rename(
        &self,
        request: RenameRequest,
        force_preview: bool,
    ) -> ContractResult<RenamePlan> {
        validate_names(request.old_name.as_deref(), &request.new_name)?;
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
        let scope_kind = request.scope.clone().unwrap_or_default();
        let allow_multi_file = request.allow_multi_file.unwrap_or(false)
            || matches!(
                scope_kind,
                RenameScope::IndexedReferences
                    | RenameScope::Project
                    | RenameScope::BoundedMultiFile
            );
        let max_changed_files = request
            .max_changed_files
            .unwrap_or(if allow_multi_file {
                DEFAULT_MAX_CHANGED_FILES_MULTI
            } else {
                DEFAULT_MAX_CHANGED_FILES
            })
            .min(DEFAULT_MAX_CHANGED_FILES_MULTI);
        let max_occurrences = request
            .max_occurrences
            .unwrap_or(DEFAULT_MAX_OCCURRENCES)
            .min(DEFAULT_MAX_OCCURRENCES);
        let max_changed_bytes = request
            .max_changed_bytes
            .unwrap_or(DEFAULT_MAX_CHANGED_BYTES)
            .min(DEFAULT_MAX_CHANGED_BYTES);
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
        let target = self.resolve_target(&scope, &project_root, &request)?;
        let old_name = request
            .old_name
            .clone()
            .unwrap_or(target.symbol.name.clone());
        validate_names(Some(&old_name), &request.new_name)?;
        let include_low = request.include_low_confidence.unwrap_or(false);
        let mut warnings = Vec::new();
        let mut conflicts = Vec::new();
        let files =
            self.candidate_files(&scope, &target, &old_name, &scope_kind, max_occurrences)?;
        let mut occurrences = Vec::new();
        for file in files {
            let absolute = resolve_under_root(&project_root, &file.path)?;
            let content = match read_text_file(&absolute) {
                Ok(content) => content,
                Err(error) => {
                    conflicts.push(conflict(
                        "unreadable_file",
                        error.to_string(),
                        Some(file.path),
                    ));
                    continue;
                }
            };
            occurrences.extend(scan_occurrences(
                &file.path,
                &content,
                &old_name,
                &target,
                include_low,
                &mut warnings,
            ));
        }
        occurrences.sort_by(|left, right| {
            left.file_path
                .cmp(&right.file_path)
                .then(left.start_line.cmp(&right.start_line))
                .then(left.start_column.cmp(&right.start_column))
        });
        occurrences.dedup_by(|a, b| {
            a.file_path == b.file_path
                && a.start_line == b.start_line
                && a.start_column == b.start_column
        });
        if occurrences.len() > max_occurrences {
            return Err(refactor_error(
                "too_many_occurrences",
                format!(
                    "rename found {} occurrences, above max_occurrences={max_occurrences}",
                    occurrences.len()
                ),
            ));
        }
        if occurrences.is_empty() {
            return Err(refactor_error(
                "no_occurrences",
                "rename target resolved but no safe occurrences were found",
            ));
        }

        let file_edits =
            self.build_file_edits(&project_root, &occurrences, &old_name, &request.new_name)?;
        let changed_files = file_edits
            .iter()
            .map(|edit| edit.file_path.clone())
            .collect::<BTreeSet<_>>();
        if changed_files.len() > max_changed_files {
            return Err(refactor_error(
                "too_many_files",
                format!(
                    "rename would change {} files, above max_changed_files={max_changed_files}",
                    changed_files.len()
                ),
            ));
        }
        let changed_bytes = file_edits
            .iter()
            .map(|edit| edit.changed_bytes)
            .sum::<usize>();
        if changed_bytes > max_changed_bytes {
            return Err(refactor_error(
                "rename_too_large",
                format!(
                    "rename changes {changed_bytes} bytes, above max_changed_bytes={max_changed_bytes}"
                ),
            ));
        }
        detect_overlaps(&file_edits, &mut conflicts);
        validate_expected_hashes(&request, &file_edits, &mut conflicts);
        validate_expected_occurrences(&request, &occurrences, &mut conflicts);
        let patch = self.patch_for_plan(&project_root, &file_edits)?;
        let (patch, patch_warnings) = bound_patch(patch);
        warnings.extend(patch_warnings);
        let plan_id = plan_id(&project_root, &old_name, &request.new_name, &file_edits);
        let preview = RenamePreview {
            target_summary: format!(
                "{} {} in {}",
                format!("{:?}", target.symbol.kind),
                target.symbol.name,
                target.file.path
            ),
            changed_files: changed_files.into_iter().collect(),
            occurrence_count: occurrences.len(),
            changed_byte_count: changed_bytes,
            patch,
            warnings: warnings.clone(),
            conflicts: conflicts.clone(),
        };
        Ok(RenamePlan {
            plan_id,
            kind: RefactorKind::RenameSymbol,
            mode,
            dry_run,
            create_backup: request.create_backup.unwrap_or(true),
            project_path: project_root.display().to_string(),
            database_path: request.database_path.clone(),
            project_id,
            branch,
            old_name,
            new_name: request.new_name,
            scope: scope_kind,
            occurrences,
            file_edits,
            preview,
            warnings,
            conflicts,
            safety_errors: Vec::new(),
        })
    }

    fn resolve_target(
        &self,
        scope: &QueryScope,
        project_root: &Path,
        request: &RenameRequest,
    ) -> ContractResult<ResolvedRenameTarget> {
        let candidates = match request.target.kind {
            RenameTargetKind::SymbolId => {
                let id = request.target.symbol_id.as_deref().ok_or_else(|| {
                    refactor_error("missing_symbol_id", "symbol_id target requires symbol_id")
                })?;
                self.repository
                    .get_symbol(scope, &SymbolId::new(id))?
                    .into_iter()
                    .collect::<Vec<_>>()
            }
            RenameTargetKind::Symbol => {
                let name = request
                    .target
                    .symbol_name
                    .as_deref()
                    .or(request.old_name.as_deref())
                    .ok_or_else(|| {
                        refactor_error(
                            "missing_symbol_name",
                            "symbol target requires symbol_name or old_name",
                        )
                    })?;
                self.repository.find_symbols(scope, name)?
            }
            RenameTargetKind::FileOldName | RenameTargetKind::FileRange => {
                let name = request.old_name.as_deref().ok_or_else(|| {
                    refactor_error("missing_old_name", "file target rename requires old_name")
                })?;
                self.repository.find_symbols(scope, name)?
            }
        };
        let mut filtered = Vec::new();
        for symbol in candidates {
            let Some(file) = self.repository.get_file(scope, &symbol.file_id)? else {
                continue;
            };
            if let Some(path) = &request.target.file_path {
                if normalize_slashes(&file.path) != normalize_slashes(path) {
                    continue;
                }
            }
            let _ = resolve_under_root(project_root, &file.path)?;
            filtered.push((symbol, file));
        }
        match filtered.len() {
            0 => Err(refactor_error(
                "symbol_not_found",
                "rename target was not found in the index",
            )),
            1 => {
                let (symbol, file) = filtered.remove(0);
                Ok(ResolvedRenameTarget { symbol, file })
            }
            _ => Err(refactor_error(
                "ambiguous_symbol",
                format!(
                    "rename target matched multiple symbols: {}",
                    filtered
                        .iter()
                        .map(|(symbol, file)| format!(
                            "{}:{}:{}",
                            file.path, symbol.name, symbol.start_line
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }

    fn candidate_files(
        &self,
        scope: &QueryScope,
        target: &ResolvedRenameTarget,
        old_name: &str,
        rename_scope: &RenameScope,
        limit: usize,
    ) -> ContractResult<Vec<QueryFile>> {
        let mut files = BTreeMap::<String, QueryFile>::new();
        files.insert(target.file.path.clone(), target.file.clone());
        if matches!(rename_scope, RenameScope::SingleFile) {
            return Ok(files.into_values().collect());
        }
        for neighbor in self.repository.graph_neighbors(
            scope,
            &target.symbol.id,
            GraphDirection::Both,
            &[EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            0,
        )? {
            for symbol_id in [neighbor.from_symbol, neighbor.to_symbol]
                .into_iter()
                .flatten()
            {
                if let Some(symbol) = self.repository.get_symbol(scope, &symbol_id)? {
                    if let Some(file) = self.repository.get_file(scope, &symbol.file_id)? {
                        files.insert(file.path.clone(), file);
                    }
                }
            }
        }
        for hit in self.repository.fts_search(scope, old_name, limit)? {
            if let Some(file) = self.repository.get_file(scope, &hit.file_id)? {
                files.insert(file.path.clone(), file);
            }
        }
        Ok(files.into_values().collect())
    }

    fn build_file_edits(
        &self,
        project_root: &Path,
        occurrences: &[RenameOccurrence],
        old_name: &str,
        new_name: &str,
    ) -> ContractResult<Vec<FileEdit>> {
        let mut result = Vec::new();
        let by_file = occurrences.iter().fold(
            BTreeMap::<String, Vec<&RenameOccurrence>>::new(),
            |mut map, occurrence| {
                map.entry(occurrence.file_path.clone())
                    .or_default()
                    .push(occurrence);
                map
            },
        );
        for (path, occurrences) in by_file {
            let absolute = resolve_under_root(project_root, &path)?;
            let content = read_text_file(&absolute)?;
            let old_hash = stable_hash(&content);
            for occurrence in occurrences {
                let old_text = slice_for_occurrence(&content, occurrence)?;
                if old_text != old_name {
                    return Err(refactor_error(
                        "stale_occurrence",
                        format!(
                            "occurrence at {}:{} no longer matches old_name",
                            path, occurrence.start_line
                        ),
                    ));
                }
                let edit = TextEdit {
                    start_line: occurrence.start_line,
                    start_column: occurrence.start_column,
                    end_line: occurrence.end_line,
                    end_column: occurrence.end_column,
                    old_text: old_text.to_string(),
                    new_text: new_name.to_string(),
                };
                let next = apply_single_edit(&content, &edit)?;
                result.push(FileEdit {
                    file_path: path.clone(),
                    operation: EditOperation::ReplaceRange,
                    text_edit: edit,
                    old_content_hash: old_hash.clone(),
                    new_content_hash: stable_hash(&next),
                    changed_bytes: old_name.len().abs_diff(new_name.len())
                        + old_name.len().min(new_name.len()),
                    changed_lines: 1,
                });
            }
        }
        Ok(result)
    }

    fn patch_for_plan(&self, project_root: &Path, edits: &[FileEdit]) -> ContractResult<String> {
        let mut patch = String::new();
        for (path, file_edits) in group_edits_by_file(edits) {
            let absolute = resolve_under_root(project_root, &path)?;
            let old = read_text_file(&absolute)?;
            let new = apply_file_edits(&old, &file_edits)?;
            patch.push_str(&unified_diff(&path, &old, &new));
        }
        Ok(patch)
    }
}

#[derive(Debug, Clone)]
struct ResolvedRenameTarget {
    symbol: QuerySymbol,
    file: QueryFile,
}

fn validate_names(old_name: Option<&str>, new_name: &str) -> ContractResult<()> {
    if new_name.is_empty() {
        return Err(refactor_error(
            "invalid_new_name",
            "new_name cannot be empty",
        ));
    }
    if let Some(old) = old_name {
        if old.is_empty() {
            return Err(refactor_error(
                "invalid_old_name",
                "old_name cannot be empty",
            ));
        }
        if old == new_name {
            return Err(refactor_error(
                "same_name",
                "old_name and new_name must differ",
            ));
        }
    }
    if !is_identifier(new_name) {
        return Err(refactor_error(
            "invalid_new_name",
            "new_name must be a conservative identifier",
        ));
    }
    Ok(())
}

fn scan_occurrences(
    file_path: &str,
    content: &str,
    old_name: &str,
    target: &ResolvedRenameTarget,
    include_low: bool,
    warnings: &mut Vec<EditValidationWarning>,
) -> Vec<RenameOccurrence> {
    let mut occurrences = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;
        for start in identifier_positions(line, old_name) {
            let column = byte_to_column(line, start);
            let in_target_definition =
                file_path == target.file.path && line_number == target.symbol.start_line;
            let (confidence, kind, evidence) = if in_target_definition {
                (
                    RenameConfidence::High,
                    RenameOccurrenceKind::Definition,
                    "indexed symbol definition".to_string(),
                )
            } else if is_comment_or_string(line, start) {
                if include_low {
                    (
                        RenameConfidence::Low,
                        RenameOccurrenceKind::Unknown,
                        "low-confidence comment/string match included by request".to_string(),
                    )
                } else {
                    warnings.push(warning(
                        "low_confidence_excluded",
                        format!("excluded low-confidence occurrence in {file_path}:{line_number}"),
                    ));
                    continue;
                }
            } else {
                (
                    RenameConfidence::Medium,
                    RenameOccurrenceKind::Reference,
                    "bounded identifier match in evidenced file".to_string(),
                )
            };
            if confidence == RenameConfidence::Low && !include_low {
                continue;
            }
            occurrences.push(RenameOccurrence {
                file_path: file_path.to_string(),
                start_line: line_number,
                start_column: column,
                end_line: line_number,
                end_column: column + old_name.chars().count(),
                old_text: old_name.to_string(),
                kind,
                confidence,
                evidence,
            });
        }
    }
    occurrences
}

fn identifier_positions(line: &str, needle: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut offset = 0;
    while let Some(found) = line[offset..].find(needle) {
        let start = offset + found;
        let end = start + needle.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if before.is_none_or(|ch| !is_identifier_continue(ch))
            && after.is_none_or(|ch| !is_identifier_continue(ch))
        {
            positions.push(start);
        }
        offset = end;
    }
    positions
}

fn is_comment_or_string(line: &str, start: usize) -> bool {
    if line[..start].contains("//") {
        return true;
    }
    let quote_count = line[..start].chars().filter(|ch| *ch == '"').count();
    quote_count % 2 == 1
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn byte_to_column(line: &str, byte: usize) -> usize {
    line[..byte].chars().count() + 1
}

fn slice_for_occurrence<'a>(
    content: &'a str,
    occurrence: &RenameOccurrence,
) -> ContractResult<&'a str> {
    let start = offset_for_position(content, occurrence.start_line, occurrence.start_column)?;
    let end = offset_for_position(content, occurrence.end_line, occurrence.end_column)?;
    Ok(&content[start..end])
}

fn apply_single_edit(content: &str, edit: &TextEdit) -> ContractResult<String> {
    let start = offset_for_position(content, edit.start_line, edit.start_column)?;
    let end = start + edit.old_text.len();
    if end > content.len() || content[start..end] != edit.old_text {
        return Err(refactor_error(
            "stale_occurrence",
            "planned edit text does not match current file",
        ));
    }
    let mut next = String::new();
    next.push_str(&content[..start]);
    next.push_str(&edit.new_text);
    next.push_str(&content[end..]);
    Ok(next)
}

fn apply_file_edits(content: &str, edits: &[FileEdit]) -> ContractResult<String> {
    let mut next = content.to_string();
    let mut sorted = edits.to_vec();
    sorted.sort_by(|left, right| {
        right
            .text_edit
            .start_line
            .cmp(&left.text_edit.start_line)
            .then(
                right
                    .text_edit
                    .start_column
                    .cmp(&left.text_edit.start_column),
            )
    });
    for edit in sorted {
        next = apply_single_edit(&next, &edit.text_edit)?;
    }
    Ok(next)
}

fn group_edits_by_file(edits: &[FileEdit]) -> BTreeMap<String, Vec<FileEdit>> {
    let mut grouped = BTreeMap::<String, Vec<FileEdit>>::new();
    for edit in edits {
        grouped
            .entry(edit.file_path.clone())
            .or_default()
            .push(edit.clone());
    }
    for file_edits in grouped.values_mut() {
        file_edits.sort_by(|left, right| {
            left.text_edit
                .start_line
                .cmp(&right.text_edit.start_line)
                .then(
                    left.text_edit
                        .start_column
                        .cmp(&right.text_edit.start_column),
                )
        });
    }
    grouped
}

fn detect_overlaps(edits: &[FileEdit], conflicts: &mut Vec<RenameConflict>) {
    for (path, file_edits) in group_edits_by_file(edits) {
        let mut seen = HashSet::new();
        for edit in file_edits {
            let key = (edit.text_edit.start_line, edit.text_edit.start_column);
            if !seen.insert(key) {
                conflicts.push(conflict(
                    "overlapping_edit",
                    "duplicate rename edit range",
                    Some(path.clone()),
                ));
            }
        }
    }
}

fn validate_expected_hashes(
    request: &RenameRequest,
    edits: &[FileEdit],
    conflicts: &mut Vec<RenameConflict>,
) {
    let Some(expected) = &request.expected_file_hashes else {
        return;
    };
    let actual = edits
        .iter()
        .map(|edit| (edit.file_path.as_str(), edit.old_content_hash.as_str()))
        .collect::<HashMap<_, _>>();
    for hash in expected {
        if actual.get(hash.file_path.as_str()).copied() != Some(hash.content_hash.as_str()) {
            conflicts.push(conflict(
                "stale_file_hash",
                "expected file hash did not match rename plan",
                Some(hash.file_path.clone()),
            ));
        }
    }
}

fn validate_expected_occurrences(
    request: &RenameRequest,
    occurrences: &[RenameOccurrence],
    conflicts: &mut Vec<RenameConflict>,
) {
    let Some(expected) = &request.expected_occurrences else {
        return;
    };
    if expected != occurrences {
        conflicts.push(conflict(
            "stale_occurrences",
            "expected occurrences did not match current rename plan",
            None,
        ));
    }
}

fn canonical_root(path: &Path) -> ContractResult<PathBuf> {
    path.canonicalize()
        .map_err(|error| refactor_error("invalid_project_root", error.to_string()))
}

fn resolve_under_root(root: &Path, relative_or_abs: &str) -> ContractResult<PathBuf> {
    let raw = PathBuf::from(relative_or_abs);
    if relative_or_abs.contains('\0') {
        return Err(refactor_error("invalid_path", "path contains NUL"));
    }
    if raw
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(refactor_error(
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
        .map_err(|error| refactor_error("invalid_target_path", error.to_string()))?;
    if !canonical.starts_with(root) {
        return Err(refactor_error(
            "outside_project_root",
            "target path is outside project root",
        ));
    }
    Ok(canonical)
}

fn read_text_file(path: &Path) -> ContractResult<String> {
    let bytes = fs::read(path).map_err(|error| refactor_error("read_failed", error.to_string()))?;
    if bytes.contains(&0) {
        return Err(refactor_error(
            "binary_file",
            "binary files cannot be renamed",
        ));
    }
    String::from_utf8(bytes).map_err(|_| refactor_error("invalid_utf8", "file is not valid UTF-8"))
}

fn offset_for_position(content: &str, line: usize, column: usize) -> ContractResult<usize> {
    if line == 0 || column == 0 {
        return Err(refactor_error("invalid_position", "positions are 1-based"));
    }
    let starts = line_starts(content);
    if line > starts.len() {
        return Err(refactor_error("invalid_range", "line is outside file"));
    }
    let start = starts[line - 1];
    let line_end = if line < starts.len() {
        starts[line]
    } else {
        content.len()
    };
    let text = &content[start..line_end].trim_end_matches(['\r', '\n']);
    let char_count = text.chars().count();
    if column > char_count + 1 {
        return Err(refactor_error("invalid_range", "column is outside line"));
    }
    Ok(start
        + text
            .chars()
            .take(column - 1)
            .map(char::len_utf8)
            .sum::<usize>())
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

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn plan_id(root: &Path, old_name: &str, new_name: &str, edits: &[FileEdit]) -> String {
    let mut seed = format!("{}:{old_name}:{new_name}", root.display());
    for edit in edits {
        seed.push_str(&format!(
            ":{}:{}:{}:{}",
            edit.file_path,
            edit.text_edit.start_line,
            edit.text_edit.start_column,
            edit.old_content_hash
        ));
    }
    stable_hash(&seed)
}

fn unified_diff(path: &str, old: &str, new: &str) -> String {
    let old_lines = old.lines().collect::<Vec<_>>();
    let new_lines = new.lines().collect::<Vec<_>>();
    let mut diff = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -1,{} +1,{} @@\n",
        old_lines.len(),
        new_lines.len()
    );
    for line in &old_lines {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in &new_lines {
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
            "rename patch exceeded maximum preview size and was truncated",
        )],
    )
}

fn create_backup(
    root: &Path,
    file: &Path,
    relative_path: &str,
    content: &str,
) -> ContractResult<PathBuf> {
    let backup_root = root.join(".b3").join("backups").join("refactors");
    fs::create_dir_all(&backup_root)
        .map_err(|error| refactor_error("backup_failed", error.to_string()))?;
    if !file.starts_with(root) {
        return Err(refactor_error(
            "outside_project_root",
            "backup target outside root",
        ));
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let backup = backup_root.join(format!(
        "{stamp}-{}.bak",
        normalize_slashes(relative_path).replace('/', "__")
    ));
    fs::write(&backup, content)
        .map_err(|error| refactor_error("backup_failed", error.to_string()))?;
    Ok(backup)
}

fn atomic_write(path: &Path, content: &str) -> ContractResult<()> {
    let tmp = path.with_extension("b3-refactor-tmp");
    {
        let mut file = fs::File::create(&tmp)
            .map_err(|error| refactor_error("write_failed", error.to_string()))?;
        file.write_all(content.as_bytes())
            .map_err(|error| refactor_error("write_failed", error.to_string()))?;
        file.sync_all()
            .map_err(|error| refactor_error("write_failed", error.to_string()))?;
    }
    fs::rename(&tmp, path).map_err(|error| refactor_error("write_failed", error.to_string()))
}

fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> EditValidationWarning {
    EditValidationWarning {
        code: code.into(),
        message: message.into(),
    }
}

fn conflict(
    code: impl Into<String>,
    message: impl Into<String>,
    file_path: Option<String>,
) -> RenameConflict {
    RenameConflict {
        code: code.into(),
        message: message.into(),
        file_path,
    }
}

fn refactor_error(code: impl Into<String>, message: impl Into<String>) -> ContractError {
    let error = RefactorSafetyError {
        code: code.into(),
        message: message.into(),
    };
    ContractError::new(format!("{}: {}", error.code, error.message))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use b3_core::{BranchId, IndexJob, Indexer, ProjectId, RenameTarget};
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

    fn multi_file_fixture() -> (tempfile::TempDir, Fixture) {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(root.join("src").join("lib.rs"), "pub fn old_name() {}\n").expect("lib");
        fs::write(
            root.join("src").join("other.rs"),
            "pub fn caller() { old_name(); }\n",
        )
        .expect("other");
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

    fn rename_request(root: &Path, new_name: &str) -> RenameRequest {
        RenameRequest {
            project_path: Some(root.display().to_string()),
            database_path: None,
            project_id: Some("default".to_string()),
            branch: Some("main".to_string()),
            target: RenameTarget {
                kind: RenameTargetKind::Symbol,
                symbol_id: None,
                symbol_name: Some("old_name".to_string()),
                file_path: Some("src/lib.rs".to_string()),
                start_line: None,
                start_column: None,
                end_line: None,
                end_column: None,
            },
            old_name: Some("old_name".to_string()),
            new_name: new_name.to_string(),
            scope: Some(RenameScope::SingleFile),
            mode: Some(EditMode::Preview),
            dry_run: None,
            create_backup: None,
            allow_multi_file: None,
            include_low_confidence: None,
            max_changed_files: None,
            max_changed_bytes: None,
            max_occurrences: None,
            expected_plan_id: None,
            expected_file_hashes: None,
            expected_occurrences: None,
        }
    }

    #[test]
    fn preview_symbol_rename_updates_definition_and_reference_not_comments_or_strings() {
        let (_dir, fx) = fixture(
            r#"pub fn old_name() {}
fn caller() { old_name(); }
// old_name comment
fn stringy() { let _ = "old_name"; }
"#,
        );
        let engine = RenameRefactorEngine::new(&fx.storage);
        let plan = engine
            .preview_rename(rename_request(&fx.root, "new_name"))
            .expect("plan");
        assert!(plan.dry_run);
        assert_eq!(plan.occurrences.len(), 2);
        assert!(plan.preview.patch.contains("+pub fn new_name()"));
        assert!(plan.preview.patch.contains("+fn caller() { new_name(); }"));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.code == "low_confidence_excluded"));
        assert!(fs::read_to_string(fx.root.join("src/lib.rs"))
            .expect("source")
            .contains("old_name"));
    }

    #[test]
    fn apply_symbol_rename_creates_backup_and_recommends_reindex() {
        let (_dir, fx) = fixture("pub fn old_name() {}\nfn caller() { old_name(); }\n");
        let engine = RenameRefactorEngine::new(&fx.storage);
        assert!(engine
            .apply_rename(rename_request(&fx.root, "new_name"))
            .is_err());
        let mut request = rename_request(&fx.root, "new_name");
        request.mode = Some(EditMode::Apply);
        request.dry_run = Some(false);
        let result = engine.apply_rename(request).expect("apply");
        assert!(result.applied);
        assert!(result.reindex_recommended);
        assert_eq!(result.backup_paths.len(), 1);
        let source = fs::read_to_string(fx.root.join("src/lib.rs")).expect("source");
        assert!(source.contains("new_name"));
        assert!(!source.contains("old_name()"));
    }

    #[test]
    fn bounded_multi_file_rename_uses_indexed_evidence() {
        let (_dir, fx) = multi_file_fixture();
        let engine = RenameRefactorEngine::new(&fx.storage);
        let mut request = rename_request(&fx.root, "new_name");
        request.scope = Some(RenameScope::IndexedReferences);
        request.allow_multi_file = Some(true);
        request.max_changed_files = Some(2);
        let plan = engine.preview_rename(request).expect("plan");
        assert_eq!(plan.preview.changed_files.len(), 2);
        assert!(plan
            .preview
            .changed_files
            .iter()
            .any(|path| path == "src/other.rs"));
        assert!(plan
            .preview
            .patch
            .contains("+pub fn caller() { new_name(); }"));
    }

    #[test]
    fn include_low_confidence_can_include_comments_and_strings() {
        let (_dir, fx) = fixture("// old_name\npub fn old_name() {}\n");
        let engine = RenameRefactorEngine::new(&fx.storage);
        let mut request = rename_request(&fx.root, "new_name");
        request.include_low_confidence = Some(true);
        let plan = engine.preview_rename(request).expect("plan");
        assert_eq!(plan.occurrences.len(), 2);
        assert!(plan
            .occurrences
            .iter()
            .any(|occurrence| occurrence.confidence == RenameConfidence::Low));
    }

    #[test]
    fn ambiguous_missing_invalid_and_bounds_are_rejected() {
        let (_dir, fx) = fixture("pub fn old_name() {}\nmod inner { pub fn old_name() {} }\n");
        let engine = RenameRefactorEngine::new(&fx.storage);
        let mut ambiguous = rename_request(&fx.root, "new_name");
        ambiguous.target.file_path = None;
        assert!(engine.preview_rename(ambiguous).is_err());
        let mut missing = rename_request(&fx.root, "new_name");
        missing.target.symbol_name = Some("missing".to_string());
        missing.old_name = Some("missing".to_string());
        assert!(engine.preview_rename(missing).is_err());
        assert!(engine
            .preview_rename(rename_request(&fx.root, "1bad"))
            .is_err());
        assert!(engine
            .preview_rename(rename_request(&fx.root, "old_name"))
            .is_err());
        let mut too_many = rename_request(&fx.root, "new_name");
        too_many.max_occurrences = Some(1);
        assert!(engine.preview_rename(too_many).is_err());
    }

    #[test]
    fn stale_hash_conflict_blocks_apply_before_writes() {
        let (_dir, fx) = fixture("pub fn old_name() {}\nfn caller() { old_name(); }\n");
        let engine = RenameRefactorEngine::new(&fx.storage);
        let plan = engine
            .preview_rename(rename_request(&fx.root, "new_name"))
            .expect("plan");
        let mut request = rename_request(&fx.root, "new_name");
        request.mode = Some(EditMode::Apply);
        request.dry_run = Some(false);
        request.expected_plan_id = Some(plan.plan_id);
        fs::write(fx.root.join("src/lib.rs"), "pub fn old_name() {}\n").expect("mutate");
        assert!(engine.apply_rename(request).is_err());
    }

    #[test]
    fn outside_root_binary_and_invalid_utf8_are_rejected_or_conflicted() {
        let (_dir, fx) = fixture("pub fn old_name() {}\n");
        fs::write(fx.root.join("src/binary.bin"), [0, 1, 2]).expect("binary");
        let engine = RenameRefactorEngine::new(&fx.storage);
        let outside = tempdir().expect("outside");
        let outside_file = outside.path().join("outside.rs");
        fs::write(&outside_file, "pub fn old_name() {}\n").expect("outside");
        let mut request = rename_request(&fx.root, "new_name");
        request.target.file_path = Some(outside_file.display().to_string());
        assert!(engine.preview_rename(request).is_err());
    }
}
