use pice_protocol::{
    InitializeResult, JsonRpcRequest, JsonRpcResponse, RequestId,
    SessionCreateResult,
};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn workspace_root() -> std::path::PathBuf {
    // Integration tests run from the workspace root when using `cargo test`
    // from the workspace, but from the crate root when using `-p pice-cli`.
    // Use CARGO_MANIFEST_DIR to find the crate, then navigate to workspace root.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().parent().unwrap().to_path_buf()
}

fn spawn_stub_provider() -> std::process::Child {
    let root = workspace_root();
    Command::new("node")
        .arg(root.join("packages/provider-stub/dist/bin.js"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn stub provider - did you run `pnpm build`?")
}

fn send_request(stdin: &mut impl Write, id: u64, method: &str, params: serde_json::Value) {
    let req = JsonRpcRequest::new(RequestId::Number(id), method, Some(params));
    let json = serde_json::to_string(&req).unwrap();
    writeln!(stdin, "{json}").unwrap();
    stdin.flush().unwrap();
}

fn read_response(reader: &mut BufReader<impl std::io::Read>) -> JsonRpcResponse {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[test]
fn provider_initialize_and_capabilities() {
    let mut child = spawn_stub_provider();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize
    send_request(&mut stdin, 1, "initialize", json!({"config": {}}));
    let resp = read_response(&mut reader);

    assert_eq!(resp.id, RequestId::Number(1));

    let result: InitializeResult = serde_json::from_value(resp.result).unwrap();
    assert_eq!(result.version, "0.1.0");
    assert!(!result.capabilities.workflow);
    assert!(!result.capabilities.evaluation);

    // Clean up
    drop(stdin);
    child.wait().ok();
}

#[test]
fn provider_session_lifecycle() {
    let mut child = spawn_stub_provider();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize first
    send_request(&mut stdin, 1, "initialize", json!({"config": {}}));
    let _init_resp = read_response(&mut reader);

    // Create session
    send_request(
        &mut stdin,
        2,
        "session/create",
        json!({"workingDirectory": "/tmp/test"}),
    );
    let session_resp = read_response(&mut reader);
    let session: SessionCreateResult = serde_json::from_value(session_resp.result).unwrap();
    assert!(session.session_id.starts_with("stub-session-"));

    // Send message — expect notifications then response
    send_request(
        &mut stdin,
        3,
        "session/send",
        json!({"sessionId": session.session_id, "message": "hello"}),
    );

    // Read lines — there will be 2 notifications + 1 response
    let mut lines = Vec::new();
    for _ in 0..3 {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        lines.push(line.trim().to_string());
    }

    // First should be response/chunk notification
    let chunk: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(chunk["method"], "response/chunk");
    assert_eq!(chunk["params"]["text"], "hello");

    // Second should be response/complete notification
    let complete: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(complete["method"], "response/complete");

    // Third should be the actual response
    let send_resp: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();
    assert_eq!(send_resp["id"], 3);
    assert_eq!(send_resp["result"]["ok"], true);

    // Clean up
    drop(stdin);
    child.wait().ok();
}

#[test]
fn provider_shutdown() {
    let mut child = spawn_stub_provider();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    send_request(&mut stdin, 1, "initialize", json!({"config": {}}));
    let _init_resp = read_response(&mut reader);

    // Shutdown
    send_request(&mut stdin, 2, "shutdown", json!(null));
    let shutdown_resp = read_response(&mut reader);
    assert_eq!(shutdown_resp.id, RequestId::Number(2));

    // Process should exit cleanly
    let status = child.wait().unwrap();
    assert!(status.success());
}
