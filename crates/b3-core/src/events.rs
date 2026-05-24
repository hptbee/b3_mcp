//! Event contracts for background work, MCP calls, config reloads, and token
//! savings accounting.

use crate::{BranchId, FileId, ProjectId, SessionId, ToolCallId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    IndexStarted(IndexStarted),
    FileDiscovered(FileDiscovered),
    FileParsed(FileParsed),
    FileSkipped(FileSkipped),
    ParserCrashed(ParserCrashed),
    ParserWorkerStarted(ParserWorkerStarted),
    ParserWorkerCompleted(ParserWorkerCompleted),
    ParserWorkerTimeout(ParserWorkerTimeout),
    ParserWorkerCrashed(ParserWorkerCrashed),
    ParseFailed(ParseFailed),
    ParseRetried(ParseRetried),
    ParseFailureRecorded(ParseFailureRecorded),
    GraphUpdated(GraphUpdated),
    QueryExecuted(QueryExecuted),
    ToolCalled(ToolCalled),
    TokensSaved(TokensSaved),
    ConfigReloaded(ConfigReloaded),
    IndexCompleted(IndexCompleted),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStarted {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub root_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiscovered {
    pub project_id: ProjectId,
    pub file_id: FileId,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileParsed {
    pub project_id: ProjectId,
    pub file_id: FileId,
    pub symbols_found: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSkipped {
    pub project_id: ProjectId,
    pub file_id: Option<FileId>,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserCrashed {
    pub project_id: ProjectId,
    pub file_id: Option<FileId>,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserWorkerStarted {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub attempt: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserWorkerCompleted {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserWorkerTimeout {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub timeout_ms: u64,
    pub attempt: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserWorkerCrashed {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub exit_code: Option<i32>,
    pub stderr_excerpt: String,
    pub attempt: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailed {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub error_kind: String,
    pub error_message: String,
    pub retry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRetried {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub attempt: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailureRecorded {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub path: String,
    pub error_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphUpdated {
    pub project_id: ProjectId,
    pub nodes_changed: usize,
    pub edges_changed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExecuted {
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub returned_tokens: usize,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCalled {
    pub tool_call_id: ToolCallId,
    pub session_id: Option<SessionId>,
    pub tool_name: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokensSaved {
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub estimated_tokens_saved: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigReloaded {
    pub project_id: Option<ProjectId>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexCompleted {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub files_seen: usize,
    pub files_parsed: usize,
}
