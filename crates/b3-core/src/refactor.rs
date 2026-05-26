use serde::{Deserialize, Serialize};

use crate::{EditMode, EditValidationWarning, FileEdit};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorKind {
    RenameSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameTargetKind {
    SymbolId,
    Symbol,
    FileRange,
    FileOldName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameTarget {
    pub kind: RenameTargetKind,
    pub symbol_id: Option<String>,
    pub symbol_name: Option<String>,
    pub file_path: Option<String>,
    pub start_line: Option<usize>,
    pub start_column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameScope {
    SingleFile,
    IndexedReferences,
    Project,
    BoundedMultiFile,
}

impl Default for RenameScope {
    fn default() -> Self {
        Self::SingleFile
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameRequest {
    pub project_path: Option<String>,
    pub database_path: Option<String>,
    pub project_id: Option<String>,
    pub branch: Option<String>,
    pub target: RenameTarget,
    pub old_name: Option<String>,
    pub new_name: String,
    pub scope: Option<RenameScope>,
    pub mode: Option<EditMode>,
    pub dry_run: Option<bool>,
    pub create_backup: Option<bool>,
    pub allow_multi_file: Option<bool>,
    pub include_low_confidence: Option<bool>,
    pub max_changed_files: Option<usize>,
    pub max_changed_bytes: Option<usize>,
    pub max_occurrences: Option<usize>,
    pub expected_plan_id: Option<String>,
    pub expected_file_hashes: Option<Vec<ExpectedFileHash>>,
    pub expected_occurrences: Option<Vec<RenameOccurrence>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedFileHash {
    pub file_path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameOccurrenceKind {
    Definition,
    Reference,
    Import,
    Export,
    Call,
    TypeReference,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameOccurrence {
    pub file_path: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub old_text: String,
    pub kind: RenameOccurrenceKind,
    pub confidence: RenameConfidence,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameConflict {
    pub code: String,
    pub message: String,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorSafetyError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePreview {
    pub target_summary: String,
    pub changed_files: Vec<String>,
    pub occurrence_count: usize,
    pub changed_byte_count: usize,
    pub patch: String,
    pub warnings: Vec<EditValidationWarning>,
    pub conflicts: Vec<RenameConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePlan {
    pub plan_id: String,
    pub kind: RefactorKind,
    pub mode: EditMode,
    pub dry_run: bool,
    pub create_backup: bool,
    pub project_path: String,
    pub database_path: Option<String>,
    pub project_id: String,
    pub branch: String,
    pub old_name: String,
    pub new_name: String,
    pub scope: RenameScope,
    pub occurrences: Vec<RenameOccurrence>,
    pub file_edits: Vec<FileEdit>,
    pub preview: RenamePreview,
    pub warnings: Vec<EditValidationWarning>,
    pub conflicts: Vec<RenameConflict>,
    pub safety_errors: Vec<RefactorSafetyError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameApplyRequest {
    #[serde(flatten)]
    pub request: RenameRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameApplyResult {
    pub applied: bool,
    pub dry_run: bool,
    pub plan_id: String,
    pub changed_files: Vec<String>,
    pub backup_paths: Vec<String>,
    pub patch: String,
    pub warnings: Vec<EditValidationWarning>,
    pub reindex_recommended: bool,
}
