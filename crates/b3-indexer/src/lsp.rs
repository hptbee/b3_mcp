//! Local LSP backend foundation.
//!
//! LSP complements tree-sitter indexing. This module only manages configured
//! local stdio language-server processes and minimal JSON-RPC/LSP messages.

use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use b3_core::{ContractError, ContractResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LspBackendStatus {
    Disabled,
    NotConfigured,
    Unavailable,
    Available,
    Running,
    Crashed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspBackend {
    pub enabled: bool,
    pub servers: Vec<LspServerConfig>,
    pub timeout: LspTimeoutConfig,
}

impl Default for LspBackend {
    fn default() -> Self {
        Self {
            enabled: false,
            servers: Vec::new(),
            timeout: LspTimeoutConfig::default(),
        }
    }
}

impl From<&b3_core::LspConfig> for LspBackend {
    fn from(config: &b3_core::LspConfig) -> Self {
        Self {
            enabled: config.enabled,
            servers: config.servers.iter().map(LspServerConfig::from).collect(),
            timeout: LspTimeoutConfig {
                startup_timeout_ms: config.startup_timeout_ms,
                request_timeout_ms: config.request_timeout_ms,
                stderr_capture_bytes: config.stderr_capture_bytes,
            },
        }
    }
}

impl LspBackend {
    pub fn status(&self) -> LspBackendStatus {
        if !self.enabled {
            LspBackendStatus::Disabled
        } else if self.servers.is_empty() {
            LspBackendStatus::NotConfigured
        } else {
            LspBackendStatus::Available
        }
    }

    pub fn server_statuses(&self) -> Vec<LspServerStatus> {
        self.servers
            .iter()
            .map(|server| LspServerStatus {
                language_id: server.language_id.clone(),
                command: server.command.program.clone(),
                enabled: server.enabled,
                status: if !self.enabled || !server.enabled {
                    LspBackendStatus::Disabled
                } else if server.command.validate_local().is_err() {
                    LspBackendStatus::Unavailable
                } else if command_path_is_explicit(&server.command.program)
                    && !Path::new(&server.command.program).exists()
                {
                    LspBackendStatus::Unavailable
                } else {
                    LspBackendStatus::Available
                },
            })
            .collect()
    }

    pub fn server_for_language(&self, language_id: &str) -> ContractResult<&LspServerConfig> {
        if !self.enabled {
            return Err(LspBackendError::Disabled.into());
        }
        self.servers
            .iter()
            .find(|server| server.language_id == language_id && server.enabled)
            .ok_or_else(|| ContractError::from(LspBackendError::ServerNotConfigured))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspServerStatus {
    pub language_id: String,
    pub command: String,
    pub enabled: bool,
    pub status: LspBackendStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub language_id: String,
    pub command: LspServerCommand,
    pub enabled: bool,
}

impl From<&b3_core::LspServerConfig> for LspServerConfig {
    fn from(config: &b3_core::LspServerConfig) -> Self {
        Self {
            language_id: config.language_id.clone(),
            command: LspServerCommand::new(config.command.clone(), config.args.clone()),
            enabled: config.enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspServerCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl LspServerCommand {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    pub fn validate_local(&self) -> ContractResult<()> {
        if self.program.trim().is_empty() {
            return Err(LspBackendError::ServerNotConfigured.into());
        }
        if self.program.contains("://") {
            return Err(LspBackendError::InvalidCommand(
                "language server command must be a local executable".to_string(),
            )
            .into());
        }
        if self.program.contains('|') || self.program.contains('&') || self.program.contains(';') {
            return Err(LspBackendError::InvalidCommand(
                "shell operators are not allowed in language server commands".to_string(),
            )
            .into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspTimeoutConfig {
    pub startup_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub stderr_capture_bytes: usize,
}

impl Default for LspTimeoutConfig {
    fn default() -> Self {
        Self {
            startup_timeout_ms: 5_000,
            request_timeout_ms: 5_000,
            stderr_capture_bytes: 4_096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspWorkspace {
    pub root_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDocument {
    pub path: PathBuf,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

impl LspDocument {
    pub fn open(path: impl Into<PathBuf>, language_id: impl Into<String>) -> ContractResult<Self> {
        let path = path.into();
        if !path.exists() {
            return Err(LspBackendError::FileNotFound.into());
        }
        let text = fs::read_to_string(&path).map_err(to_contract_error)?;
        Ok(Self {
            path,
            language_id: language_id.into(),
            version: 1,
            text,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LspRequestId {
    Number(u64),
}

impl LspRequestId {
    fn as_value(self) -> Value {
        match self {
            Self::Number(value) => json!(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LspRequest {
    pub id: LspRequestId,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LspResponse {
    pub id: LspRequestId,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LspNotification {
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub range: LspRange,
    pub severity: Option<u8>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LspCapabilitySet {
    pub definition_provider: bool,
    pub references_provider: bool,
    pub implementation_provider: bool,
    pub diagnostics: bool,
    pub text_document_sync: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspBackendError {
    Disabled,
    ServerNotConfigured,
    ServerBinaryNotFound,
    StartupTimeout,
    InitializeFailed,
    RequestTimeout,
    ProcessCrashed,
    InvalidResponse,
    UnsupportedCapability,
    FileNotFound,
    UriPathConversion,
    InvalidCommand(String),
}

impl std::fmt::Display for LspBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("LSP backend is disabled"),
            Self::ServerNotConfigured => formatter.write_str("LSP server is not configured"),
            Self::ServerBinaryNotFound => formatter.write_str("LSP server binary was not found"),
            Self::StartupTimeout => formatter.write_str("LSP server startup timed out"),
            Self::InitializeFailed => formatter.write_str("LSP initialize failed"),
            Self::RequestTimeout => formatter.write_str("LSP request timed out"),
            Self::ProcessCrashed => formatter.write_str("LSP server process crashed"),
            Self::InvalidResponse => formatter.write_str("LSP server returned an invalid response"),
            Self::UnsupportedCapability => formatter.write_str("LSP capability is unsupported"),
            Self::FileNotFound => formatter.write_str("document file was not found"),
            Self::UriPathConversion => formatter.write_str("could not convert path to file URI"),
            Self::InvalidCommand(message) => formatter.write_str(message),
        }
    }
}

impl From<LspBackendError> for ContractError {
    fn from(value: LspBackendError) -> Self {
        ContractError::new(value.to_string())
    }
}

pub struct LspServerProcess {
    child: Child,
    incoming: Receiver<ContractResult<Value>>,
    stderr_excerpt: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
    stdout_thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for LspServerProcess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LspServerProcess")
            .field("stderr_excerpt", &self.stderr_excerpt())
            .finish_non_exhaustive()
    }
}

impl LspServerProcess {
    pub fn start(
        backend_enabled: bool,
        config: &LspServerConfig,
        timeout: &LspTimeoutConfig,
    ) -> ContractResult<Self> {
        if !backend_enabled || !config.enabled {
            return Err(LspBackendError::Disabled.into());
        }
        config.command.validate_local()?;
        if command_path_is_explicit(&config.command.program)
            && !Path::new(&config.command.program).exists()
        {
            return Err(LspBackendError::ServerBinaryNotFound.into());
        }

        let mut command = Command::new(&config.command.program);
        command
            .args(&config.command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ContractError::from(LspBackendError::ServerBinaryNotFound)
            } else {
                to_contract_error(error)
            }
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ContractError::from(LspBackendError::ProcessCrashed))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ContractError::from(LspBackendError::ProcessCrashed))?;

        let (sender, incoming) = mpsc::channel();
        let stdout_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_lsp_message(&mut reader) {
                    Ok(message) => {
                        if sender.send(Ok(message)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        });

        let stderr_excerpt = Arc::new(Mutex::new(String::new()));
        let stderr_capture = Arc::clone(&stderr_excerpt);
        let capture_bytes = timeout.stderr_capture_bytes;
        let stderr_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buffer = [0_u8; 256];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        let chunk = String::from_utf8_lossy(&buffer[..count]);
                        let mut excerpt = stderr_capture.lock().expect("stderr lock");
                        excerpt.push_str(&chunk);
                        if excerpt.len() > capture_bytes {
                            let drain_to = excerpt.len() - capture_bytes;
                            excerpt.drain(..drain_to);
                        }
                    }
                }
            }
        });

        let mut process = Self {
            child,
            incoming,
            stderr_excerpt,
            stderr_thread: Some(stderr_thread),
            stdout_thread: Some(stdout_thread),
        };
        if timeout.startup_timeout_ms == 0 {
            let _ = process.stop();
            return Err(LspBackendError::StartupTimeout.into());
        }
        if !process.is_running()? {
            return Err(LspBackendError::ProcessCrashed.into());
        }
        Ok(process)
    }

    pub fn is_running(&mut self) -> ContractResult<bool> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(to_contract_error)
    }

    pub fn send(&mut self, payload: &Value) -> ContractResult<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| ContractError::from(LspBackendError::ProcessCrashed))?;
        stdin
            .write_all(encode_lsp_message(payload).as_bytes())
            .map_err(to_contract_error)?;
        stdin.flush().map_err(to_contract_error)
    }

    pub fn request(
        &mut self,
        payload: &Value,
        request_id: LspRequestId,
        timeout: Duration,
    ) -> ContractResult<Value> {
        self.send(payload)?;
        self.response_for(request_id, timeout)
    }

    pub fn response_for(
        &mut self,
        request_id: LspRequestId,
        timeout: Duration,
    ) -> ContractResult<Value> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self
                .incoming
                .recv_timeout(remaining.min(Duration::from_millis(50)))
            {
                Ok(Ok(message)) => {
                    if message.get("method").and_then(Value::as_str)
                        == Some("textDocument/publishDiagnostics")
                    {
                        continue;
                    }
                    if message.get("id") == Some(&request_id.as_value()) {
                        if let Some(error) = message.get("error") {
                            return Err(ContractError::new(format!("LSP request failed: {error}")));
                        }
                        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
                    }
                }
                Ok(Err(error)) => return Err(error),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !self.is_running()? {
                        return Err(LspBackendError::ProcessCrashed.into());
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(LspBackendError::ProcessCrashed.into());
                }
            }
        }
        Err(LspBackendError::RequestTimeout.into())
    }

    pub fn next_message(&mut self, timeout: Duration) -> ContractResult<Value> {
        match self.incoming.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(LspBackendError::RequestTimeout.into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(LspBackendError::ProcessCrashed.into())
            }
        }
    }

    pub fn stop(&mut self) -> ContractResult<()> {
        let _ = self.send(&json!({"jsonrpc":"2.0","id":999999u64,"method":"shutdown"}));
        let _ = self.send(&json!({"jsonrpc":"2.0","method":"exit"}));
        let _ = self.child.stdin.take();
        if self.is_running()? {
            self.child.kill().map_err(to_contract_error)?;
        }
        let _ = self.child.wait();
        if let Some(handle) = self.stdout_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    pub fn stderr_excerpt(&self) -> String {
        self.stderr_excerpt
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }
}

impl Drop for LspServerProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub struct LspClient {
    next_id: u64,
    diagnostics: HashMap<String, Vec<LspDiagnostic>>,
}

impl Default for LspClient {
    fn default() -> Self {
        Self {
            next_id: 1,
            diagnostics: HashMap::new(),
        }
    }
}

impl LspClient {
    pub fn initialize(
        &mut self,
        process: &mut LspServerProcess,
        workspace: &LspWorkspace,
        timeout: Duration,
    ) -> ContractResult<LspCapabilitySet> {
        let (id, request) = self.initialize_request(workspace)?;
        let result = process.request(&request, id, timeout)?;
        let capabilities = parse_capabilities(&result);
        process.send(&self.initialized_notification())?;
        Ok(capabilities)
    }

    pub fn open_document(
        &mut self,
        process: &mut LspServerProcess,
        document: &LspDocument,
    ) -> ContractResult<()> {
        process.send(&self.did_open_notification(document)?)
    }

    pub fn definition(
        &mut self,
        process: &mut LspServerProcess,
        path: &Path,
        position: LspPosition,
        timeout: Duration,
    ) -> ContractResult<Vec<LspLocation>> {
        let (id, request) = self.definition_request(path, position)?;
        let result = process.request(&request, id, timeout)?;
        parse_locations(&result)
    }

    pub fn references(
        &mut self,
        process: &mut LspServerProcess,
        path: &Path,
        position: LspPosition,
        timeout: Duration,
    ) -> ContractResult<Vec<LspLocation>> {
        let (id, request) = self.references_request(path, position)?;
        let result = process.request(&request, id, timeout)?;
        parse_locations(&result)
    }

    pub fn implementations(
        &mut self,
        process: &mut LspServerProcess,
        path: &Path,
        position: LspPosition,
        timeout: Duration,
    ) -> ContractResult<Vec<LspLocation>> {
        let (id, request) = self.implementation_request(path, position)?;
        let result = process.request(&request, id, timeout)?;
        parse_locations(&result)
    }

    pub fn collect_diagnostics(
        &mut self,
        process: &mut LspServerProcess,
        timeout: Duration,
    ) -> ContractResult<Vec<LspDiagnostic>> {
        let message = process.next_message(timeout)?;
        let uri = message
            .get("params")
            .and_then(|params| params.get("uri"))
            .and_then(Value::as_str)
            .ok_or_else(|| ContractError::from(LspBackendError::InvalidResponse))?
            .to_string();
        let diagnostics = parse_diagnostics(&message)?;
        self.diagnostics.insert(uri, diagnostics.clone());
        Ok(diagnostics)
    }

    pub fn cached_diagnostics(&self, uri: &str) -> Vec<LspDiagnostic> {
        self.diagnostics.get(uri).cloned().unwrap_or_default()
    }

    pub fn initialize_request(
        &mut self,
        workspace: &LspWorkspace,
    ) -> ContractResult<(LspRequestId, Value)> {
        let id = self.next_request_id();
        Ok((
            id,
            json!({
                "jsonrpc": "2.0",
                "id": id.as_value(),
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": path_to_uri(&workspace.root_path)?,
                    "capabilities": {}
                }
            }),
        ))
    }

    pub fn initialized_notification(&self) -> Value {
        json!({"jsonrpc":"2.0","method":"initialized","params":{}})
    }

    pub fn did_open_notification(&self, document: &LspDocument) -> ContractResult<Value> {
        Ok(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": path_to_uri(&document.path)?,
                    "languageId": document.language_id,
                    "version": document.version,
                    "text": document.text
                }
            }
        }))
    }

    pub fn did_change_notification(&self, document: &LspDocument) -> ContractResult<Value> {
        Ok(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": path_to_uri(&document.path)?,
                    "version": document.version
                },
                "contentChanges": [{ "text": document.text }]
            }
        }))
    }

    pub fn definition_request(
        &mut self,
        path: &Path,
        position: LspPosition,
    ) -> ContractResult<(LspRequestId, Value)> {
        self.text_document_position_request("textDocument/definition", path, position)
    }

    pub fn references_request(
        &mut self,
        path: &Path,
        position: LspPosition,
    ) -> ContractResult<(LspRequestId, Value)> {
        let (id, mut request) =
            self.text_document_position_request("textDocument/references", path, position)?;
        request["params"]["context"] = json!({"includeDeclaration": true});
        Ok((id, request))
    }

    pub fn implementation_request(
        &mut self,
        path: &Path,
        position: LspPosition,
    ) -> ContractResult<(LspRequestId, Value)> {
        self.text_document_position_request("textDocument/implementation", path, position)
    }

    fn text_document_position_request(
        &mut self,
        method: &str,
        path: &Path,
        position: LspPosition,
    ) -> ContractResult<(LspRequestId, Value)> {
        let id = self.next_request_id();
        Ok((
            id,
            json!({
                "jsonrpc": "2.0",
                "id": id.as_value(),
                "method": method,
                "params": {
                    "textDocument": { "uri": path_to_uri(path)? },
                    "position": position
                }
            }),
        ))
    }

    fn next_request_id(&mut self) -> LspRequestId {
        let id = self.next_id;
        self.next_id += 1;
        LspRequestId::Number(id)
    }
}

pub fn encode_lsp_message(payload: &Value) -> String {
    let body = payload.to_string();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

pub fn decode_lsp_message(input: &str) -> ContractResult<Value> {
    let (_headers, body) = input
        .split_once("\r\n\r\n")
        .ok_or_else(|| ContractError::from(LspBackendError::InvalidResponse))?;
    serde_json::from_str(body).map_err(to_contract_error)
}

pub fn parse_capabilities(initialize_result: &Value) -> LspCapabilitySet {
    let capabilities = initialize_result
        .get("capabilities")
        .unwrap_or(initialize_result);
    LspCapabilitySet {
        definition_provider: provider_is_enabled(capabilities.get("definitionProvider")),
        references_provider: provider_is_enabled(capabilities.get("referencesProvider")),
        implementation_provider: provider_is_enabled(capabilities.get("implementationProvider")),
        diagnostics: true,
        text_document_sync: capabilities.get("textDocumentSync").cloned(),
    }
}

pub fn parse_diagnostics(notification: &Value) -> ContractResult<Vec<LspDiagnostic>> {
    let diagnostics = notification
        .get("params")
        .and_then(|params| params.get("diagnostics"))
        .and_then(Value::as_array)
        .ok_or_else(|| ContractError::from(LspBackendError::InvalidResponse))?;
    diagnostics
        .iter()
        .map(|diagnostic| serde_json::from_value(diagnostic.clone()).map_err(to_contract_error))
        .collect()
}

pub fn parse_locations(value: &Value) -> ContractResult<Vec<LspLocation>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .map(|location| serde_json::from_value(location.clone()).map_err(to_contract_error))
            .collect();
    }
    if value.get("targetUri").is_some() {
        return Ok(vec![LspLocation {
            uri: value
                .get("targetUri")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            range: serde_json::from_value(
                value
                    .get("targetSelectionRange")
                    .or_else(|| value.get("targetRange"))
                    .cloned()
                    .ok_or_else(|| ContractError::from(LspBackendError::InvalidResponse))?,
            )
            .map_err(to_contract_error)?,
        }]);
    }
    serde_json::from_value(value.clone())
        .map(|location| vec![location])
        .map_err(to_contract_error)
}

pub fn path_to_uri(path: &Path) -> ContractResult<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(to_contract_error)?
            .join(path)
    };
    let text = absolute.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        return Err(LspBackendError::UriPathConversion.into());
    }
    Ok(format!(
        "file:///{}",
        percent_encode_path(text.trim_start_matches('/'))
    ))
}

fn read_lsp_message(reader: &mut impl BufRead) -> ContractResult<Value> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let count = reader.read_line(&mut line).map_err(to_contract_error)?;
        if count == 0 {
            return Err(LspBackendError::ProcessCrashed.into());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| ContractError::from(LspBackendError::InvalidResponse))?,
            );
        }
    }

    let length =
        content_length.ok_or_else(|| ContractError::from(LspBackendError::InvalidResponse))?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).map_err(to_contract_error)?;
    serde_json::from_slice(&body).map_err(to_contract_error)
}

fn provider_is_enabled(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => false,
    }
}

fn percent_encode_path(path: &str) -> String {
    path.replace('%', "%25").replace(' ', "%20")
}

fn command_path_is_explicit(program: &str) -> bool {
    program.contains('/') || program.contains('\\')
}

fn to_contract_error(error: impl std::fmt::Display) -> ContractError {
    ContractError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_backend_defaults_disabled() {
        let backend = LspBackend::default();
        assert_eq!(backend.status(), LspBackendStatus::Disabled);
        assert!(backend.servers.is_empty());
    }

    #[test]
    fn core_config_maps_to_backend_config() {
        let config = b3_core::LspConfig {
            enabled: true,
            startup_timeout_ms: 1_000,
            request_timeout_ms: 2_000,
            stderr_capture_bytes: 128,
            servers: vec![b3_core::LspServerConfig::local_disabled(
                "typescript",
                "typescript-language-server",
            )],
        };
        let backend = LspBackend::from(&config);
        assert!(backend.enabled);
        assert_eq!(backend.timeout.request_timeout_ms, 2_000);
        assert_eq!(backend.servers[0].language_id, "typescript");
        assert!(!backend.servers[0].enabled);
    }

    #[test]
    fn validates_local_server_commands() {
        assert!(
            LspServerCommand::new("typescript-language-server", Vec::new())
                .validate_local()
                .is_ok()
        );
        assert!(
            LspServerCommand::new("https://example.com/server", Vec::new())
                .validate_local()
                .is_err()
        );
        assert!(LspServerCommand::new("server | powershell", Vec::new())
            .validate_local()
            .is_err());
    }

    #[test]
    fn frames_and_decodes_json_rpc_messages() {
        let payload = json!({"jsonrpc":"2.0","id":1,"method":"initialize"});
        let framed = encode_lsp_message(&payload);
        assert!(framed.starts_with("Content-Length:"));
        assert_eq!(decode_lsp_message(&framed).expect("decode"), payload);
    }

    #[test]
    fn builds_initialize_and_document_requests() {
        let mut client = LspClient::default();
        let workspace = LspWorkspace {
            root_path: PathBuf::from("."),
        };
        let (_id, initialize) = client.initialize_request(&workspace).expect("initialize");
        assert_eq!(initialize["method"], "initialize");

        let document = LspDocument {
            path: PathBuf::from("src/lib.rs"),
            language_id: "rust".to_string(),
            version: 1,
            text: "fn main() {}".to_string(),
        };
        let did_open = client.did_open_notification(&document).expect("did open");
        assert_eq!(did_open["method"], "textDocument/didOpen");
        let (_id, definition) = client
            .definition_request(
                &document.path,
                LspPosition {
                    line: 0,
                    character: 3,
                },
            )
            .expect("definition");
        assert_eq!(definition["method"], "textDocument/definition");
        let (_id, references) = client
            .references_request(
                &document.path,
                LspPosition {
                    line: 0,
                    character: 3,
                },
            )
            .expect("references");
        assert_eq!(references["method"], "textDocument/references");
    }

    #[test]
    fn parses_capabilities_locations_and_diagnostics() {
        let capabilities = parse_capabilities(&json!({
            "capabilities": {
                "definitionProvider": true,
                "referencesProvider": true,
                "implementationProvider": {"documentSelector": null},
                "textDocumentSync": 1
            }
        }));
        assert!(capabilities.definition_provider);
        assert!(capabilities.references_provider);
        assert!(capabilities.implementation_provider);

        let location = json!({
            "uri": "file:///tmp/a.ts",
            "range": {
                "start": {"line": 1, "character": 2},
                "end": {"line": 1, "character": 4}
            }
        });
        assert_eq!(parse_locations(&location).expect("location").len(), 1);

        let diagnostics = parse_diagnostics(&json!({
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": "file:///tmp/a.ts",
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 2},
                        "end": {"line": 1, "character": 4}
                    },
                    "severity": 1,
                    "message": "missing semicolon"
                }]
            }
        }))
        .expect("diagnostics");
        assert_eq!(diagnostics[0].message, "missing semicolon");
    }

    #[test]
    fn missing_or_disabled_server_returns_clear_error() {
        let config = LspServerConfig {
            language_id: "typescript".to_string(),
            command: LspServerCommand::new("definitely-missing-b3-lsp", Vec::new()),
            enabled: true,
        };
        let error = LspServerProcess::start(true, &config, &LspTimeoutConfig::default())
            .expect_err("missing server");
        assert!(error.message.contains("binary"));
        let disabled = LspServerProcess::start(false, &config, &LspTimeoutConfig::default())
            .expect_err("disabled");
        assert!(disabled.message.contains("disabled"));
    }
}
