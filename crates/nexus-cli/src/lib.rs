//! Shared CLI dispatcher and local JSON-RPC/MCP transports.
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use nexus_hardware::{Acquisition, SerialConfig};
use nexus_script::Session;
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    sync::{Arc, Mutex},
};

#[derive(Parser)]
#[command(
    name = "nexuslab",
    version,
    about = "Local numerical workspace, live scope, and automation API"
)]
pub struct Arguments {
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Subcommand)]
pub enum Command {
    /// Run a .nx, .m, or .rhai script without opening a window.
    Run {
        file: std::path::PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Evaluate one expression without opening a window.
    Eval {
        code: String,
        #[arg(long)]
        json: bool,
    },
    /// Serve localhost HTTP JSON-RPC/MCP, or newline-delimited MCP on stdin/stdout.
    Serve {
        #[arg(long, default_value_t = 8080)]
        port: u16,
        #[arg(long)]
        stdio: bool,
    },
    /// List available serial and USB devices.
    Devices,
}
pub fn dispatch() -> Result<bool, Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    let Some(command) = arguments.command else {
        return Ok(false);
    };
    match command {
        Command::Run { file, json } => {
            let metadata = std::fs::metadata(&file)?;
            if metadata.len() > 65536 {
                return Err("script exceeds 64 KiB".into());
            }
            let mut source = std::fs::read_to_string(&file)?;
            if file.extension().is_some_and(|s| s == "rhai") {
                source = format!("rhai: {source}");
            }
            evaluate(&source, json)?;
        }
        Command::Eval { code, json } => evaluate(&code, json)?,
        Command::Devices => println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"serial":nexus_hardware::ports()?,"usb":nexus_hardware::usb_devices()?})
            )?
        ),
        Command::Serve { port, stdio } => {
            let state = Arc::new(WorkspaceService::new());
            if stdio {
                serve_stdio(state)?;
            } else {
                tokio::runtime::Runtime::new()?.block_on(serve_http(port, state))?;
            }
        }
    }
    Ok(true)
}
fn evaluate(source: &str, as_json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let result = Session::new().evaluate(source)?;
    if as_json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}\nDone ({:.3} ms)", result.output, result.elapsed_ms);
    }
    Ok(())
}
pub struct WorkspaceService {
    session: Mutex<Session>,
    hardware: Mutex<Acquisition>,
}
impl Default for WorkspaceService {
    fn default() -> Self {
        Self::new()
    }
}
impl WorkspaceService {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(Session::new()),
            hardware: Mutex::new(Acquisition::start_mock()),
        }
    }
    pub fn rpc(&self, request: Value) -> Option<Value> {
        if let Value::Array(batch) = request {
            if batch.is_empty() {
                return Some(error(Value::Null, -32600, "Empty batch"));
            }
            let results: Vec<_> = batch
                .into_iter()
                .filter_map(|r| self.rpc_single(r))
                .collect();
            return if results.is_empty() {
                None
            } else {
                Some(Value::Array(results))
            };
        }
        self.rpc_single(request)
    }
    fn rpc_single(&self, request: Value) -> Option<Value> {
        if request.is_object()
            && request["jsonrpc"] == "2.0"
            && request.get("id").is_some()
            && request.get("method").is_none()
            && (request.get("result").is_some() || request.get("error").is_some())
        {
            return None;
        }
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if !request.is_object()
            || request["jsonrpc"] != "2.0"
            || !request["method"].is_string()
            || (!id.is_null() && !id.is_string() && !id.is_number())
        {
            return Some(error(Value::Null, -32600, "Invalid Request"));
        }
        let method = request["method"].as_str().unwrap();
        let notification = request.get("id").is_none();
        let params = &request["params"];
        if !params.is_null() && !params.is_object() {
            return if notification {
                None
            } else {
                Some(error(id, -32602, "params must be an object"))
            };
        }
        let result: Result<Value, (i32, String)> = match method {
            "initialize" => Ok(
                json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{},"resources":{}},"serverInfo":{"name":"nexuslab","version":env!("CARGO_PKG_VERSION")}}),
            ),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools":tools()})),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("");
                let args = &params["arguments"];
                match self.invoke(name, args) {
                    Ok(v) => Ok(
                        json!({"content":[{"type":"text","text":v.to_string()}],"isError":false}),
                    ),
                    Err(e) => Ok(json!({"content":[{"type":"text","text":e}],"isError":true})),
                }
            }
            "resources/list" => Ok(
                json!({"resources":[{"uri":"nexus://workspace","name":"Numerical workspace","mimeType":"application/json"}]}),
            ),
            "resources/read" => {
                if params["uri"] == "nexus://workspace" {
                    Ok(
                        json!({"contents":[{"uri":"nexus://workspace","mimeType":"application/json","text":self.session.lock().unwrap().variables_to_json().to_string()}]}),
                    )
                } else {
                    Err((-32602, "Unknown resource".into()))
                }
            }
            "execute" | "inspect" | "list_ports" | "read_samples" | "connect" | "disconnect" => {
                self.invoke(method, params).map_err(|e| (-32000, e))
            }
            _ => Err((-32601, format!("Unknown method: {method}"))),
        };
        if notification {
            return None;
        }
        Some(match result {
            Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
            Err((code, message)) => error(id, code, &message),
        })
    }
    fn invoke(&self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "execute" => {
                let code = args["code"].as_str().ok_or("code must be a string")?;
                let result = self
                    .session
                    .lock()
                    .unwrap()
                    .evaluate(code)
                    .map_err(|e| e.to_string())?;
                serde_json::to_value(result).map_err(|e| e.to_string())
            }
            "inspect" => {
                let s = self.session.lock().unwrap();
                if let Some(name) = args["name"].as_str() {
                    s.variables
                        .get(name)
                        .map(|v| json!(v))
                        .ok_or_else(|| format!("Unknown variable {name}"))
                } else {
                    Ok(s.variables_to_json())
                }
            }
            "list_ports" => Ok(
                json!({"serial":nexus_hardware::ports().map_err(|e|e.to_string())?,"usb":nexus_hardware::usb_devices().map_err(|e|e.to_string())?}),
            ),
            "read_samples" => {
                let limit = args["limit"].as_u64().unwrap_or(1024).min(16384) as usize;
                let mut h = self.hardware.lock().unwrap();
                let mut samples = Vec::with_capacity(limit);
                h.drain(&mut samples, limit);
                Ok(
                    json!({"samples":samples,"status":h.status(),"sample_rate":h.sample_rate(),"dropped":h.dropped.load(std::sync::atomic::Ordering::Relaxed)}),
                )
            }
            "connect" => {
                let port = args["port"].as_str().ok_or("port is required")?;
                let baud = args["baud"].as_u64().unwrap_or(115200);
                if baud == 0 || baud > u32::MAX as u64 {
                    return Err("baud must fit a positive u32".into());
                }
                let sample_rate = args["sample_rate"]
                    .as_f64()
                    .unwrap_or(nexus_hardware::SAMPLE_RATE);
                if !sample_rate.is_finite() || sample_rate <= 0.0 {
                    return Err("sample_rate must be finite and positive".into());
                }
                let framing = match args["framing"].as_str().unwrap_or("lines") {
                    "lines" => nexus_hardware::Framing::Lines,
                    "float32le" => nexus_hardware::Framing::Float32Le,
                    _ => return Err("framing must be lines or float32le".into()),
                };
                use nexus_hardware::serialport_export::{FlowControl, Parity};
                let parity = match args["parity"].as_str().unwrap_or("none") {
                    "none" => Parity::None,
                    "odd" => Parity::Odd,
                    "even" => Parity::Even,
                    _ => return Err("invalid parity".into()),
                };
                let flow_control = match args["flow_control"].as_str().unwrap_or("none") {
                    "none" => FlowControl::None,
                    "software" => FlowControl::Software,
                    "hardware" => FlowControl::Hardware,
                    _ => return Err("invalid flow control".into()),
                };
                self.hardware.lock().unwrap().connect(SerialConfig {
                    port: port.into(),
                    baud: baud as u32,
                    sample_rate,
                    framing,
                    parity,
                    flow_control,
                });
                Ok(json!({"status":"connecting"}))
            }
            "disconnect" => {
                self.hardware.lock().unwrap().disconnect();
                Ok(json!({"status":"disconnected"}))
            }
            _ => Err(format!("Unknown tool: {name}")),
        }
    }
}
trait WorkspaceJson {
    fn variables_to_json(&self) -> Value;
}
impl WorkspaceJson for Session {
    fn variables_to_json(&self) -> Value {
        json!(self.variables)
    }
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn tools() -> Value {
    json!([
        {"name":"execute","description":"Evaluate bounded MATLAB-style syntax or rhai: code in the persistent workspace","inputSchema":{"type":"object","properties":{"code":{"type":"string"}},"required":["code"]}},
        {"name":"inspect","description":"Inspect one variable or the full workspace","inputSchema":{"type":"object","properties":{"name":{"type":"string"}}}},
        {"name":"list_ports","description":"Enumerate serial and USB devices","inputSchema":{"type":"object","properties":{}}},
        {"name":"read_samples","description":"Drain buffered mock or hardware samples","inputSchema":{"type":"object","properties":{"limit":{"type":"integer","minimum":0,"maximum":16384}}}},
        {"name":"connect","description":"Start a serial worker with automatic reconnect","inputSchema":{"type":"object","properties":{"port":{"type":"string"},"baud":{"type":"integer"},"sample_rate":{"type":"number"}},"required":["port"]}},
        {"name":"disconnect","description":"Disconnect acquisition","inputSchema":{"type":"object","properties":{}}}
    ])
}
async fn handle(
    State(service): State<Arc<WorkspaceService>>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    if let Some(origin) = headers.get("origin") {
        let allowed = origin
            .to_str()
            .ok()
            .and_then(|s| s.parse::<axum::http::Uri>().ok())
            .is_some_and(|uri| {
                matches!(uri.scheme_str(), Some("http" | "https"))
                    && matches!(uri.host(), Some("127.0.0.1" | "localhost"))
                    && uri.path() == "/"
                    && uri.query().is_none()
            });
        if !allowed {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    match tokio::task::spawn_blocking(move || service.rpc(request)).await {
        Ok(Some(response)) => Json(response).into_response(),
        Ok(None) => StatusCode::ACCEPTED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
async fn serve_http(
    port: u16,
    state: Arc<WorkspaceService>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ok","version":env!("CARGO_PKG_VERSION")})) }),
        )
        .route("/rpc", post(handle))
        .route("/mcp", post(handle))
        .layer(DefaultBodyLimit::max(131072))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    eprintln!("NexusLab listening at http://127.0.0.1:{port}/mcp (JSON-RPC: /rpc)");
    axum::serve(listener, app).await?;
    Ok(())
}
fn serve_stdio(service: Arc<WorkspaceService>) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = if line.len() > 131072 {
            Some(error(Value::Null, -32600, "Request exceeds 128 KiB"))
        } else {
            match serde_json::from_str(&line) {
                Ok(request) => service.rpc(request),
                Err(_) => Some(error(Value::Null, -32700, "Parse error")),
            }
        };
        if let Some(response) = response {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rpc_persists_variables_and_mcp_tools() {
        let a = WorkspaceService::new();
        let r = a
            .rpc(json!({"jsonrpc":"2.0","id":1,"method":"execute","params":{"code":"x = 2 + 3"}}))
            .unwrap();
        assert!(r.get("error").is_none());
        let r = a
            .rpc(json!({"jsonrpc":"2.0","id":2,"method":"inspect","params":{"name":"x"}}))
            .unwrap();
        assert_eq!(r["result"]["value"]["data"][0], 5.0);
        let r=a.rpc(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"execute","arguments":{"code":"y = x*2"}}})).unwrap();
        assert_eq!(r["result"]["isError"], false);
        assert!(a
            .rpc(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .is_none());
        assert_eq!(
            a.rpc(json!({"jsonrpc":"2.0","id":4,"method":"bad"}))
                .unwrap()["error"]["code"],
            -32601
        );
    }
}
