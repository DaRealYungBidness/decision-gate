// system-tests/tests/helpers/stdio_client.rs
// ============================================================================
// Module: MCP Stdio Client
// Description: JSON-RPC stdio client for Decision Gate MCP servers.
// Purpose: Exercise the MCP stdio transport with real framing.
// Dependencies: decision-gate-contract, serde
// ============================================================================

use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;

use decision_gate_contract::tooling::ToolDefinition;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use super::mcp_client::TranscriptEntry;

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ToolListResult {
    tools: Vec<ToolDefinition>,
}

#[derive(Debug, Deserialize)]
struct ToolCallResult {
    content: Vec<ToolContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolContent {
    Json { json: Value },
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Option<Value>,
}

/// Stdio MCP client for integration testing.
pub struct StdioMcpClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    transcript: Arc<Mutex<Vec<TranscriptEntry>>>,
    next_id: Arc<Mutex<u64>>,
}

impl StdioMcpClient {
    /// Spawns a stdio MCP server process and connects the client.
    pub fn spawn(binary: &Path, config_path: &Path, stderr_path: &Path) -> Result<Self, String> {
        let stderr_file = std::fs::File::create(stderr_path)
            .map_err(|err| format!("failed to create stderr log: {err}"))?;
        let mut command = Command::new(binary);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr_file))
            .env("DECISION_GATE_CONFIG", config_path);

        let mut child = command.spawn().map_err(|err| format!("spawn failed: {err}"))?;
        let stdin = child.stdin.take().ok_or_else(|| "missing child stdin".to_string())?;
        let stdout = child.stdout.take().ok_or_else(|| "missing child stdout".to_string())?;

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            transcript: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        })
    }

    /// Returns a snapshot of transcript entries.
    pub fn transcript(&self) -> Vec<TranscriptEntry> {
        self.transcript.lock().map_or_else(|_| Vec::new(), |entries| entries.clone())
    }

    /// Issues a tools/list request.
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>, String> {
        let response = self.send_request("tools/list", None).await?;
        let result =
            response.result.ok_or_else(|| "missing result in tools/list response".to_string())?;
        let parsed: ToolListResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid tools/list payload: {err}"))?;
        Ok(parsed.tools)
    }

    /// Issues a tools/call request and returns the tool JSON payload.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let response = self.send_request("tools/call", Some(params)).await?;
        let result = response.result.ok_or_else(|| format!("missing result for tool {name}"))?;
        let parsed: ToolCallResult = serde_json::from_value(result)
            .map_err(|err| format!("invalid tools/call payload for {name}: {err}"))?;
        let json = parsed
            .content
            .into_iter()
            .find_map(|item| match item {
                ToolContent::Json {
                    json,
                } => Some(json),
            })
            .ok_or_else(|| format!("tool {name} returned no json content"))?;
        Ok(json)
    }

    async fn send_request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse, String> {
        let stdin = Arc::clone(&self.stdin);
        let stdout = Arc::clone(&self.stdout);
        let transcript = Arc::clone(&self.transcript);
        let next_id = Arc::clone(&self.next_id);
        let method = method.to_string();

        tokio::task::spawn_blocking(move || {
            let id = {
                let mut guard = next_id.lock().map_err(|_| "id lock poisoned".to_string())?;
                let value = *guard;
                *guard = value.saturating_add(1);
                value
            };
            let request = JsonRpcRequest {
                jsonrpc: "2.0",
                id,
                method: method.clone(),
                params,
            };
            let payload = serde_json::to_vec(&request)
                .map_err(|err| format!("jsonrpc serialization failed: {err}"))?;

            {
                let mut input = stdin.lock().map_err(|_| "stdin lock poisoned".to_string())?;
                write_framed(&mut *input, &payload)?;
            }

            let response_bytes = {
                let mut output = stdout.lock().map_err(|_| "stdout lock poisoned".to_string())?;
                read_framed(&mut *output)?
            };

            let response: JsonRpcResponse = serde_json::from_slice(&response_bytes)
                .map_err(|err| format!("invalid json-rpc response: {err}"))?;

            record_transcript(&transcript, &request, &response);

            if let Some(error) = response.error.as_ref() {
                return Err(error.message.clone());
            }
            Ok(response)
        })
        .await
        .map_err(|err| format!("stdio request join failed: {err}"))?
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn record_transcript(
    transcript: &Arc<Mutex<Vec<TranscriptEntry>>>,
    request: &JsonRpcRequest,
    response: &JsonRpcResponse,
) {
    let mut guard = match transcript.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let sequence = guard.len() as u64 + 1;
    guard.push(TranscriptEntry {
        sequence,
        method: request.method.clone(),
        request: serde_json::to_value(request).unwrap_or(Value::Null),
        response: serde_json::to_value(response).unwrap_or(Value::Null),
        error: response.error.as_ref().map(|err| err.message.clone()),
    });
}

fn read_framed(reader: &mut BufReader<impl Read>) -> Result<Vec<u8>, String> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes =
            reader.read_line(&mut line).map_err(|err| format!("stdio read failed: {err}"))?;
        if bytes == 0 {
            return Err("stdio closed".to_string());
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            let parsed =
                value.trim().parse::<usize>().map_err(|_| "invalid content length".to_string())?;
            content_length = Some(parsed);
        }
    }
    let len = content_length.ok_or_else(|| "missing content length".to_string())?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).map_err(|err| format!("stdio read failed: {err}"))?;
    Ok(buf)
}

fn write_framed(writer: &mut impl Write, payload: &[u8]) -> Result<(), String> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    writer.write_all(header.as_bytes()).map_err(|err| format!("stdio write failed: {err}"))?;
    writer.write_all(payload).map_err(|err| format!("stdio write failed: {err}"))?;
    writer.flush().map_err(|err| format!("stdio write failed: {err}"))?;
    Ok(())
}
