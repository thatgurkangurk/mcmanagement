use askama::Template;
use axum::{
    Router,
    extract::{
        Path, State,
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use notify::{RecursiveMode, Result as NotifyResult, Watcher};
use serde::Deserialize;
use std::{
    fs,
    sync::{Arc, RwLock},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::protocol::{Message as TungsteniteMessage, Role},
};
use url::Url;

#[derive(Deserialize, Clone)]
struct ServerConfig {
    id: String,
    name: String,
    url: String,
    password: String,
}

#[derive(Deserialize, Clone)]
struct Config {
    servers: Vec<ServerConfig>,
}

struct AppState {
    config: RwLock<Config>,
}

// --- Askama Templates ---
#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    servers: Vec<ServerConfig>,
}

#[derive(Template)]
#[template(path = "console.html")]
struct ConsoleTemplate {
    server: ServerConfig,
}

#[tokio::main]
async fn main() {
    let config_path = option_env!("RUNNING_IN_DOCKER")
        .map(|_| "/app/servers.json")
        .unwrap_or("servers.json");

    let config: Config = serde_json::from_str(&fs::read_to_string(config_path).unwrap()).unwrap();
    let state = Arc::new(AppState {
        config: RwLock::new(config),
    });

    // 3. Setup File Watcher
    let watcher_state = state.clone();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                // Reload the file
                if let Ok(new_data) = fs::read_to_string(config_path) {
                    if let Ok(new_cfg) = serde_json::from_str::<Config>(&new_data) {
                        let mut cfg = watcher_state.config.write().unwrap();
                        *cfg = new_cfg;
                        println!("Configuration reloaded!");
                    }
                }
            }
        }
    })
    .unwrap();

    watcher
        .watch(config_path.as_ref(), RecursiveMode::NonRecursive)
        .unwrap();

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/server/{id}", get(serve_console))
        .route("/ws/{id}", get(ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Manager listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn serve_index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let template = IndexTemplate {
        servers: state.config.read().unwrap().servers.clone(),
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            eprintln!("Failed to render index template: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template Error").into_response()
        }
    }
}

async fn serve_console(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let server = state
        .config
        .read()
        .unwrap()
        .servers
        .iter()
        .find(|s| s.id == id)
        .cloned();

    match server {
        Some(s) => {
            let template = ConsoleTemplate { server: s };
            match template.render() {
                Ok(html) => Html(html).into_response(),
                Err(e) => {
                    eprintln!("Failed to render console template: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Template Error").into_response()
                }
            }
        }
        None => Html("<h1>404 - Server Not Found</h1>".to_string()).into_response(),
    }
}

async fn ws_handler(
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, id))
}

async fn handle_socket(browser_ws: WebSocket, state: Arc<AppState>, server_id: String) {
    let server_config = {
        let config = state.config.read().unwrap();
        match config.servers.iter().find(|s| s.id == server_id).cloned() {
            Some(c) => c,
            None => {
                eprintln!("WebSocket requested for unknown server ID: {}", server_id);
                return;
            }
        }
    };

    let parsed_url = Url::parse(&server_config.url).expect("Invalid URL in config");
    let host = parsed_url.host_str().unwrap_or("localhost");
    let port = parsed_url.port_or_known_default().unwrap_or(80);
    let path = if parsed_url.path().is_empty() {
        "/"
    } else {
        parsed_url.path()
    };
    let addr = format!("{}:{}", host, port);

    let mut stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to TCP: {}", e);
            return;
        }
    };

    // use a dummy (but compliant) WebSocket key
    // the server will accept it (it never even checks it)
    let request = format!(
        "GET {} HTTP/1.1\r\n\
        Host: {}\r\n\
        Upgrade: websocket\r\n\
        Connection: Upgrade\r\n\
        Sec-WebSocket-Version: 13\r\n\
        Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
        Sec-WebSocket-Protocol: mc-server-runner-ws-v1, {}\r\n\
        \r\n",
        path, addr, server_config.password
    );

    if let Err(e) = stream.write_all(request.as_bytes()).await {
        eprintln!("Failed to send handshake: {}", e);
        return;
    }

    let mut reader = BufReader::new(stream);
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        if let Err(e) = reader.read_line(&mut line).await {
            eprintln!("Failed to read handshake: {}", e);
            return;
        }
        headers.push_str(&line);
        if line == "\r\n" {
            break;
        }
    }

    if !headers.starts_with("HTTP/1.1 101") {
        eprintln!("Handshake rejected by server:\n{}", headers);
        return;
    }

    let mc_ws = WebSocketStream::from_raw_socket(reader, Role::Client, None).await;

    let (mut mc_sender, mut mc_receiver) = mc_ws.split();
    let (mut browser_sender, mut browser_receiver) = browser_ws.split();

    let mut mc_to_browser = tokio::spawn(async move {
        while let Some(Ok(msg)) = mc_receiver.next().await {
            match msg {
                TungsteniteMessage::Text(text) => {
                    let axum_text = AxumMessage::Text(text.to_string().into());
                    if browser_sender.send(axum_text).await.is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });

    let mut browser_to_mc = tokio::spawn(async move {
        while let Some(Ok(msg)) = browser_receiver.next().await {
            if let AxumMessage::Text(text) = msg {
                let ts_text = TungsteniteMessage::Text(text.to_string().into());
                if mc_sender.send(ts_text).await.is_err() {
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut mc_to_browser) => browser_to_mc.abort(),
        _ = (&mut browser_to_mc) => mc_to_browser.abort(),
    }
}
