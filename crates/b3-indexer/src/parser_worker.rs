use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use b3_core::{
    BranchId, DomainEvent, EdgeConfidence, EdgeId, EventBus, FileId, GraphEdgeMetadata,
    ParseRetried, ParserWorkerCompleted, ParserWorkerCrashed, ParserWorkerStarted,
    ParserWorkerTimeout, ProjectId, SymbolId,
};
use serde::{Deserialize, Serialize};

use crate::{
    edge_kind_name, edge_provenance_name, node_kind_name, parse_edge_kind, parse_edge_provenance,
    parse_node_kind, DefaultLanguagePack, ExtractedRelationship, ExtractedSymbol, IndexerConfig,
    ParseInput, ParsedFile, ParserIsolation, TreeSitterParser,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserJobRequest {
    pub project_id: String,
    pub branch_id: String,
    pub file_id: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserJobResponse {
    pub file_id: String,
    pub language: Option<String>,
    pub symbols: Vec<ParserSymbolDto>,
    pub relationships: Vec<ParserRelationshipDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserSymbolDto {
    pub id: String,
    pub file_id: String,
    pub name: String,
    pub kind: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserRelationshipDto {
    pub id: String,
    pub from_symbol: String,
    pub to_symbol: String,
    pub kind: String,
    pub confidence_bps: u16,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserErrorDto {
    pub kind: String,
    pub message: String,
    pub stderr_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ParserWorkerOutput {
    Parsed(ParserJobResponse),
    Failed(ParserErrorDto),
}

impl ParserJobRequest {
    fn from_input(project_id: &ProjectId, branch_id: &BranchId, input: &ParseInput) -> Self {
        Self {
            project_id: project_id.as_str().to_string(),
            branch_id: branch_id.as_str().to_string(),
            file_id: input.file_id.as_str().to_string(),
            path: input.path.to_string_lossy().to_string(),
            source: input.source.clone(),
        }
    }
}

impl From<ParsedFile> for ParserJobResponse {
    fn from(value: ParsedFile) -> Self {
        Self {
            file_id: value.file_id.as_str().to_string(),
            language: value.language,
            symbols: value
                .symbols
                .into_iter()
                .map(ParserSymbolDto::from)
                .collect(),
            relationships: value
                .relationships
                .into_iter()
                .map(ParserRelationshipDto::from)
                .collect(),
        }
    }
}

impl From<ExtractedSymbol> for ParserSymbolDto {
    fn from(value: ExtractedSymbol) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            file_id: value.file_id.as_str().to_string(),
            name: value.name,
            kind: node_kind_name(value.kind).to_string(),
            start_byte: value.start_byte,
            end_byte: value.end_byte,
            start_line: value.start_line,
            start_column: value.start_column,
            end_line: value.end_line,
            end_column: value.end_column,
            visibility: value.visibility,
        }
    }
}

impl From<ParserSymbolDto> for ExtractedSymbol {
    fn from(value: ParserSymbolDto) -> Self {
        Self {
            id: SymbolId::new(value.id),
            file_id: FileId::new(value.file_id),
            name: value.name,
            kind: parse_node_kind(&value.kind),
            start_byte: value.start_byte,
            end_byte: value.end_byte,
            start_line: value.start_line,
            start_column: value.start_column,
            end_line: value.end_line,
            end_column: value.end_column,
            visibility: value.visibility,
        }
    }
}

impl From<ExtractedRelationship> for ParserRelationshipDto {
    fn from(value: ExtractedRelationship) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            from_symbol: value.from_symbol.as_str().to_string(),
            to_symbol: value.to_symbol.as_str().to_string(),
            kind: edge_kind_name(value.kind).to_string(),
            confidence_bps: value.metadata.confidence.basis_points(),
            provenance: edge_provenance_name(value.metadata.provenance).to_string(),
        }
    }
}

impl From<ParserRelationshipDto> for ExtractedRelationship {
    fn from(value: ParserRelationshipDto) -> Self {
        Self {
            id: EdgeId::new(value.id),
            from_symbol: SymbolId::new(value.from_symbol),
            to_symbol: SymbolId::new(value.to_symbol),
            kind: parse_edge_kind(&value.kind),
            metadata: GraphEdgeMetadata {
                confidence: EdgeConfidence::from_basis_points(value.confidence_bps),
                provenance: parse_edge_provenance(&value.provenance),
                created_at_unix_ms: 0,
                updated_at_unix_ms: 0,
            },
        }
    }
}

impl From<ParserJobResponse> for ParsedFile {
    fn from(value: ParserJobResponse) -> Self {
        Self {
            file_id: FileId::new(value.file_id),
            language: value.language,
            symbols: value
                .symbols
                .into_iter()
                .map(ExtractedSymbol::from)
                .collect(),
            relationships: value
                .relationships
                .into_iter()
                .map(ExtractedRelationship::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserFailureKind {
    ParseError,
    Timeout,
    WorkerCrash,
    WorkerIo,
}

impl ParserFailureKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ParseError => "parse_error",
            Self::Timeout => "timeout",
            Self::WorkerCrash => "worker_crash",
            Self::WorkerIo => "worker_io",
        }
    }

    pub(crate) fn retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::WorkerCrash | Self::WorkerIo)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserFailure {
    pub kind: ParserFailureKind,
    pub message: String,
    pub stderr_excerpt: Option<String>,
    pub exit_code: Option<i32>,
    pub retry_count: usize,
}

impl ParserFailure {
    fn parse_error(error: impl Into<String>) -> Self {
        Self {
            kind: ParserFailureKind::ParseError,
            message: error.into(),
            stderr_excerpt: None,
            exit_code: None,
            retry_count: 0,
        }
    }

    pub(crate) fn timeout(timeout_ms: u64) -> Self {
        Self {
            kind: ParserFailureKind::Timeout,
            message: format!("parser worker exceeded {timeout_ms}ms timeout"),
            stderr_excerpt: None,
            exit_code: None,
            retry_count: 0,
        }
    }

    pub(crate) fn worker_crash(exit_code: Option<i32>, stderr_excerpt: String) -> Self {
        Self {
            kind: ParserFailureKind::WorkerCrash,
            message: "parser worker exited unsuccessfully".to_string(),
            stderr_excerpt: Some(stderr_excerpt),
            exit_code,
            retry_count: 0,
        }
    }

    fn worker_io(error: impl Into<String>) -> Self {
        Self {
            kind: ParserFailureKind::WorkerIo,
            message: error.into(),
            stderr_excerpt: None,
            exit_code: None,
            retry_count: 0,
        }
    }
}

pub struct ParserWorkerManager<'a, P, B> {
    parser: &'a P,
    event_bus: &'a B,
    config: &'a IndexerConfig,
}

impl<'a, P, B> ParserWorkerManager<'a, P, B>
where
    P: TreeSitterParser,
    B: EventBus,
{
    pub fn new(parser: &'a P, event_bus: &'a B, config: &'a IndexerConfig) -> Self {
        Self {
            parser,
            event_bus,
            config,
        }
    }

    pub fn parse(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        input: ParseInput,
        path_for_events: &str,
    ) -> Result<ParsedFile, ParserFailure> {
        let mut last_failure = None;
        let max_attempts = self.config.parser_max_retries.saturating_add(1);

        for attempt in 0..max_attempts {
            let _ = self
                .event_bus
                .publish(DomainEvent::ParserWorkerStarted(ParserWorkerStarted {
                    project_id: project_id.clone(),
                    branch_id: branch_id.clone(),
                    file_id: input.file_id.clone(),
                    path: path_for_events.to_string(),
                    attempt,
                }));
            let started = Instant::now();
            let result = match self.config.parser_isolation {
                ParserIsolation::InProcess => self.parse_in_process(input.clone()),
                ParserIsolation::SubprocessWorker => {
                    self.parse_in_subprocess(project_id, branch_id, input.clone())
                }
            };

            match result {
                Ok(parsed) => {
                    let _ = self.event_bus.publish(DomainEvent::ParserWorkerCompleted(
                        ParserWorkerCompleted {
                            project_id: project_id.clone(),
                            branch_id: branch_id.clone(),
                            file_id: input.file_id.clone(),
                            path: path_for_events.to_string(),
                            elapsed_ms: started.elapsed().as_millis() as u64,
                        },
                    ));
                    return Ok(parsed);
                }
                Err(mut failure) => {
                    failure.retry_count = attempt;
                    self.publish_failure_event(
                        project_id,
                        branch_id,
                        &input,
                        path_for_events,
                        &failure,
                        attempt,
                    );
                    let should_retry = failure.kind.retryable() && attempt + 1 < max_attempts;
                    if should_retry {
                        let _ = self
                            .event_bus
                            .publish(DomainEvent::ParseRetried(ParseRetried {
                                project_id: project_id.clone(),
                                branch_id: branch_id.clone(),
                                file_id: input.file_id.clone(),
                                path: path_for_events.to_string(),
                                attempt: attempt + 1,
                                reason: failure.message.clone(),
                            }));
                        last_failure = Some(failure);
                        continue;
                    }
                    return Err(failure);
                }
            }
        }

        Err(last_failure.unwrap_or_else(|| ParserFailure::parse_error("parser failed")))
    }

    fn parse_in_process(&self, input: ParseInput) -> Result<ParsedFile, ParserFailure> {
        self.parser
            .parse(input)
            .map_err(|error| ParserFailure::parse_error(error.message))
    }

    fn parse_in_subprocess(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        input: ParseInput,
    ) -> Result<ParsedFile, ParserFailure> {
        let worker_path = self
            .config
            .parser_worker_path
            .clone()
            .or_else(default_parser_worker_path)
            .ok_or_else(|| ParserFailure::worker_io("parser worker path could not be resolved"))?;
        let request = ParserJobRequest::from_input(project_id, branch_id, &input);
        run_parser_worker(
            &worker_path,
            request,
            Duration::from_millis(self.config.parser_timeout_ms),
        )
    }

    fn publish_failure_event(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        input: &ParseInput,
        path_for_events: &str,
        failure: &ParserFailure,
        attempt: usize,
    ) {
        match failure.kind {
            ParserFailureKind::Timeout => {
                let _ =
                    self.event_bus
                        .publish(DomainEvent::ParserWorkerTimeout(ParserWorkerTimeout {
                            project_id: project_id.clone(),
                            branch_id: branch_id.clone(),
                            file_id: input.file_id.clone(),
                            path: path_for_events.to_string(),
                            timeout_ms: self.config.parser_timeout_ms,
                            attempt,
                        }));
            }
            ParserFailureKind::WorkerCrash => {
                let _ =
                    self.event_bus
                        .publish(DomainEvent::ParserWorkerCrashed(ParserWorkerCrashed {
                            project_id: project_id.clone(),
                            branch_id: branch_id.clone(),
                            file_id: input.file_id.clone(),
                            path: path_for_events.to_string(),
                            exit_code: failure.exit_code,
                            stderr_excerpt: failure.stderr_excerpt.clone().unwrap_or_default(),
                            attempt,
                        }));
            }
            ParserFailureKind::ParseError | ParserFailureKind::WorkerIo => {}
        }
    }
}

pub fn parse_worker_json_line(line: &str) -> ParserWorkerOutput {
    match serde_json::from_str::<ParserJobRequest>(line) {
        Ok(request) => {
            let parser = DefaultLanguagePack;
            let input = ParseInput {
                file_id: FileId::new(request.file_id),
                path: PathBuf::from(request.path),
                source: request.source,
            };
            match parser.parse(input) {
                Ok(parsed) => ParserWorkerOutput::Parsed(ParserJobResponse::from(parsed)),
                Err(error) => ParserWorkerOutput::Failed(ParserErrorDto {
                    kind: "parse_error".to_string(),
                    message: error.message,
                    stderr_excerpt: None,
                }),
            }
        }
        Err(error) => ParserWorkerOutput::Failed(ParserErrorDto {
            kind: "invalid_request".to_string(),
            message: error.to_string(),
            stderr_excerpt: None,
        }),
    }
}

fn run_parser_worker(
    worker_path: &Path,
    request: ParserJobRequest,
    timeout: Duration,
) -> Result<ParsedFile, ParserFailure> {
    let mut child = Command::new(worker_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ParserFailure::worker_io(error.to_string()))?;

    if let Some(stdin) = child.stdin.as_mut() {
        serde_json::to_writer(&mut *stdin, &request)
            .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
        stdin
            .write_all(b"\n")
            .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
    }
    drop(child.stdin.take());

    let started = Instant::now();
    while started.elapsed() < timeout {
        match child
            .try_wait()
            .map_err(|error| ParserFailure::worker_io(error.to_string()))?
        {
            Some(_status) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
                let stderr_excerpt = bounded_excerpt(&String::from_utf8_lossy(&output.stderr));
                if !output.status.success() {
                    return Err(ParserFailure::worker_crash(
                        output.status.code(),
                        stderr_excerpt,
                    ));
                }
                let stdout = String::from_utf8(output.stdout)
                    .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
                let first_line = stdout
                    .lines()
                    .next()
                    .ok_or_else(|| ParserFailure::worker_io("parser worker returned no output"))?;
                let output = serde_json::from_str::<ParserWorkerOutput>(first_line)
                    .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
                return match output {
                    ParserWorkerOutput::Parsed(response) => Ok(ParsedFile::from(response)),
                    ParserWorkerOutput::Failed(error) => Err(ParserFailure {
                        kind: ParserFailureKind::ParseError,
                        message: error.message,
                        stderr_excerpt: error.stderr_excerpt,
                        exit_code: None,
                        retry_count: 0,
                    }),
                };
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(ParserFailure::timeout(timeout.as_millis() as u64))
}

fn default_parser_worker_path() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let suffix = std::env::consts::EXE_SUFFIX;
    Some(current.with_file_name(format!("b3-parser-worker{suffix}")))
}

fn bounded_excerpt(value: &str) -> String {
    const MAX_EXCERPT_BYTES: usize = 2048;
    value.chars().take(MAX_EXCERPT_BYTES).collect()
}
