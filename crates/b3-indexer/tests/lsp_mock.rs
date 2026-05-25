use std::{path::PathBuf, time::Duration};

use b3_indexer::lsp::{
    LspClient, LspDocument, LspPosition, LspRequestId, LspServerCommand, LspServerConfig,
    LspServerProcess, LspTimeoutConfig, LspWorkspace,
};

fn mock_server_path() -> &'static str {
    env!("CARGO_BIN_EXE_b3-mock-lsp-server")
}

fn mock_config() -> LspServerConfig {
    LspServerConfig {
        language_id: "mock".to_string(),
        command: LspServerCommand::new(mock_server_path(), Vec::new()),
        enabled: true,
    }
}

#[test]
fn mock_lsp_process_round_trips_initialize_definition_references_and_diagnostics() {
    let mut process = LspServerProcess::start(true, &mock_config(), &LspTimeoutConfig::default())
        .expect("start mock server");
    let mut client = LspClient::default();
    let capabilities = client
        .initialize(
            &mut process,
            &LspWorkspace {
                root_path: PathBuf::from("."),
            },
            Duration::from_secs(2),
        )
        .expect("initialize");
    assert!(capabilities.definition_provider);
    assert!(capabilities.references_provider);
    assert!(!capabilities.implementation_provider);

    let document = LspDocument {
        path: PathBuf::from("src/lib.rs"),
        language_id: "rust".to_string(),
        version: 1,
        text: "fn main() {}".to_string(),
    };
    client
        .open_document(&mut process, &document)
        .expect("did open");
    let diagnostics = client
        .collect_diagnostics(&mut process, Duration::from_secs(2))
        .expect("diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "mock diagnostic");

    let definitions = client
        .definition(
            &mut process,
            &document.path,
            LspPosition {
                line: 0,
                character: 3,
            },
            Duration::from_secs(2),
        )
        .expect("definition");
    assert_eq!(definitions.len(), 1);

    let references = client
        .references(
            &mut process,
            &document.path,
            LspPosition {
                line: 0,
                character: 3,
            },
            Duration::from_secs(2),
        )
        .expect("references");
    assert_eq!(references.len(), 1);
}

#[test]
fn mock_lsp_request_timeout_is_reported() {
    let mut process = LspServerProcess::start(true, &mock_config(), &LspTimeoutConfig::default())
        .expect("start mock server");
    let error = process
        .response_for(LspRequestId::Number(777), Duration::from_millis(20))
        .expect_err("timeout");
    assert!(error.message.contains("timed out"));
}

#[test]
fn zero_startup_timeout_is_reported_without_leaving_process_running() {
    let error = LspServerProcess::start(
        true,
        &mock_config(),
        &LspTimeoutConfig {
            startup_timeout_ms: 0,
            request_timeout_ms: 20,
            stderr_capture_bytes: 128,
        },
    )
    .expect_err("startup timeout");
    assert!(error.message.contains("startup timed out"));
}
