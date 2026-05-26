use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    Preview,
    Apply,
}

impl Default for EditMode {
    fn default() -> Self {
        Self::Preview
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditTargetKind {
    FileRange,
    Symbol,
    File,
    QueryResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditTarget {
    pub kind: EditTargetKind,
    pub file_path: Option<String>,
    pub symbol_name: Option<String>,
    pub symbol_id: Option<String>,
    pub query: Option<String>,
    pub start_line: Option<usize>,
    pub start_column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditOperation {
    ReplaceRange,
    ReplaceSymbolBody,
    InsertBeforeSymbol,
    InsertAfterSymbol,
    AppendToFile,
    PrependToFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditRequest {
    pub project_path: Option<String>,
    pub database_path: Option<String>,
    pub project_id: Option<String>,
    pub branch: Option<String>,
    pub target: EditTarget,
    pub operation: EditOperation,
    pub mode: Option<EditMode>,
    pub dry_run: Option<bool>,
    pub create_backup: Option<bool>,
    pub allow_multi_file: Option<bool>,
    pub max_changed_files: Option<usize>,
    pub max_changed_bytes: Option<usize>,
    pub expected_current_text: Option<String>,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEdit {
    pub file_path: String,
    pub operation: EditOperation,
    pub text_edit: TextEdit,
    pub old_content_hash: String,
    pub new_content_hash: String,
    pub changed_bytes: usize,
    pub changed_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditValidationWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditSafetyError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditPreview {
    pub file_path: String,
    pub operation: EditOperation,
    pub target_summary: String,
    pub old_snippet: String,
    pub new_snippet: String,
    pub changed_line_count: usize,
    pub changed_byte_count: usize,
    pub safety_status: String,
    pub warnings: Vec<EditValidationWarning>,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditPlan {
    pub plan_id: String,
    pub mode: EditMode,
    pub dry_run: bool,
    pub create_backup: bool,
    pub project_path: String,
    pub database_path: Option<String>,
    pub project_id: String,
    pub branch: String,
    pub target: EditTarget,
    pub operation: EditOperation,
    pub file_edits: Vec<FileEdit>,
    pub preview: EditPreview,
    pub warnings: Vec<EditValidationWarning>,
    pub safety_errors: Vec<EditSafetyError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditApplyRequest {
    pub plan_id: Option<String>,
    #[serde(flatten)]
    pub request: EditRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditApplyResult {
    pub applied: bool,
    pub dry_run: bool,
    pub plan_id: String,
    pub changed_files: Vec<String>,
    pub backup_paths: Vec<String>,
    pub patch: String,
    pub warnings: Vec<EditValidationWarning>,
}
