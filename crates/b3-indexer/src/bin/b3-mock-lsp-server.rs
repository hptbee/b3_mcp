use std::io::{BufRead, BufReader, Write};

use serde_json::{json, Value};

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Ok(message) = read_lsp_message(&mut reader) {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match method {
            "initialize" => {
                let id = message.get("id").cloned().unwrap_or(json!(1));
                write_lsp_message(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "capabilities": {
                                "definitionProvider": true,
                                "referencesProvider": true,
                                "implementationProvider": false,
                                "textDocumentSync": 1
                            }
                        }
                    }),
                );
            }
            "textDocument/didOpen" => {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("file:///tmp/mock.rs");
                write_lsp_message(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": {
                            "uri": uri,
                            "diagnostics": [{
                                "range": {
                                    "start": {"line": 0, "character": 0},
                                    "end": {"line": 0, "character": 2}
                                },
                                "severity": 1,
                                "message": "mock diagnostic"
                            }]
                        }
                    }),
                );
            }
            "textDocument/definition" | "textDocument/references" => {
                let id = message.get("id").cloned().unwrap_or(json!(1));
                write_lsp_message(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": [{
                            "uri": "file:///tmp/mock.rs",
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end": {"line": 0, "character": 2}
                            }
                        }]
                    }),
                );
            }
            "shutdown" => {
                let id = message.get("id").cloned().unwrap_or(json!(999999));
                write_lsp_message(
                    &mut writer,
                    &json!({"jsonrpc":"2.0","id": id, "result": null}),
                );
            }
            "exit" => break,
            _ => {}
        }
    }
}

fn read_lsp_message(reader: &mut impl BufRead) -> Result<Value, String> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let count = reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("stdin closed".to_string());
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
                    .map_err(|error| error.to_string())?,
            );
        }
    }

    let length = content_length.ok_or_else(|| "missing Content-Length".to_string())?;
    let mut body = vec![0_u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&body).map_err(|error| error.to_string())
}

fn write_lsp_message(writer: &mut impl Write, message: &Value) {
    let body = message.to_string();
    let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    writer.write_all(framed.as_bytes()).expect("write message");
    writer.flush().expect("flush message");
}
