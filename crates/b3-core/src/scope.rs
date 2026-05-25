//! Shared scoped-indexing contracts.
//!
//! These are data contracts only. Parsing, planning, file discovery, and index
//! mutation stay in adapter/indexer crates.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexScopeKind {
    Project,
    Path,
    File,
    Glob,
    Language,
    Framework,
    Route,
    Component,
    Module,
    DataAccess,
    Realtime,
    Messaging,
    Infrastructure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexScope {
    pub kind: IndexScopeKind,
    pub value: Option<String>,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub project_id: Option<String>,
    pub branch_id: Option<String>,
    pub dry_run: bool,
    pub force: bool,
    pub recursive: bool,
    pub limit: Option<usize>,
}

impl IndexScope {
    pub fn project() -> Self {
        Self {
            kind: IndexScopeKind::Project,
            value: None,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            language: None,
            framework: None,
            project_id: None,
            branch_id: None,
            dry_run: false,
            force: false,
            recursive: true,
            limit: None,
        }
    }

    pub fn new(kind: IndexScopeKind, value: Option<String>) -> Self {
        Self {
            kind,
            value,
            ..Self::project()
        }
    }

    pub fn display(&self) -> String {
        match (&self.kind, self.value.as_deref()) {
            (IndexScopeKind::Project, None | Some("")) => "project".to_string(),
            (IndexScopeKind::Messaging, Some(value)) => format!("messaging:{value}"),
            (kind, Some(value)) => format!("{}:{value}", scope_kind_name(kind)),
            (kind, None) => scope_kind_name(kind).to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopePreview {
    pub scope: String,
    pub matched_files: usize,
    pub sample_files: Vec<String>,
    pub matched_languages: Vec<String>,
    pub matched_frameworks: Vec<String>,
    pub estimated_symbols_affected: Option<usize>,
    pub existing_metadata_targets: Vec<String>,
    pub warnings: Vec<String>,
    pub skipped_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeError {
    pub code: String,
    pub message: String,
}

impl ScopeError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

pub fn scope_kind_name(kind: &IndexScopeKind) -> &'static str {
    match kind {
        IndexScopeKind::Project => "project",
        IndexScopeKind::Path => "path",
        IndexScopeKind::File => "file",
        IndexScopeKind::Glob => "glob",
        IndexScopeKind::Language => "language",
        IndexScopeKind::Framework => "framework",
        IndexScopeKind::Route => "route",
        IndexScopeKind::Component => "component",
        IndexScopeKind::Module => "module",
        IndexScopeKind::DataAccess => "data_access",
        IndexScopeKind::Realtime => "realtime",
        IndexScopeKind::Messaging => "messaging",
        IndexScopeKind::Infrastructure => "infrastructure",
    }
}
