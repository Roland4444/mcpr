use axum::{
    extract::Json,
    http::StatusCode,
    responce::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tracing::info;


#[tokio::main]
fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();
    let app = Router::new()
        .route("/health", get(health))
        .route("/test/mcp", post(mcp_handler))
        .route("/test/mcp", get(mcp_sse_stub));
    let addr = SocketAddr::from([0,0,0,0], 5000);
    info!("MCP serv listening on {}", addr);
    let listener = tokio::net::TpcListener::bind(addr).await.unwrap();
    axum::serve(listener, app) . await.unwrap();
           
}

async fn health() -> &'static str{
    "OK"
}

async fn mcp_sse_stub() -> impl Intorespobnse{
    (
        StatusCode::OK,
        [("content-type", "text/event-stream")],
        ": hello from rust mcp \n\n"
    )
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: option<String>,
    id: Option<value>,
    method: String,
    #[serde(default)]
    params: Value,
}


async fn mcp_handler(Json(req): Json<JsonRpcRequest>) -> Response {
    info!("MCP request: method={} id={:?}", req.method, req.id);
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        // 1. Handshake
        "initialize" => {
            let body = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "rust-mcp",
                        "version": "0.1.0"
                    }
                }
            });
            json_response(StatusCode::OK, body)
        }

        // 2. Уведомление — не отвечаем
        "notifications/initialized" => {
            (StatusCode::OK, "").into_response()
        }

        // 3. Список инструментов
        "tools/list" => {
            let tools = json!({
                "tools": [
                    {
                        "name": "ping",
                        "description": "Простой тест: возвращает 'pong' и ваш текст",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "text": { "type": "string", "description": "Текст для эха" }
                            },
                            "required": ["text"]
                        }
                    },
                    {
                        "name": "server_time",
                        "description": "Возвращает текущее время сервера",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }
                ]
            });
            json_response(StatusCode::OK, json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": tools
            }))
        }

        // 4. Вызов инструмента
        "tools/call" => {
            let name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));

            let text = match name {
                "ping" => {
                    let input = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    format!("pong: {}", input)
                }
                "server_time" => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    format!("Unix timestamp: {}", now)
                }
                _ => {
                    let err = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("Unknown tool: {}", name)
                        }
                    });
                    return json_response(StatusCode::OK, err);
                }
            };

            json_response(StatusCode::OK, json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [
                        { "type": "text", "text": text }
                    ]
                }
            }))
        }

        // 5. Неизвестный метод
        _ => {
            let body = json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Unknown method: {}", req.method)
                }
            });
            json_response(StatusCode::OK, body)
        }
    }
}

// ------------------------------------------------------------------
// Утилита: JSON-ответ
// ------------------------------------------------------------------
fn json_response(status: StatusCode, body: Value) -> Response {
    let json_string = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());
    (
        status,
        [("content-type", "application/json")],
        json_string,
    )
        .into_response()
}