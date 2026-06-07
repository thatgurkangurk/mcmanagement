use crate::config::ServerConfig;
use crate::state::AppState;
use askama::Template;
use axum::{
    extract::{
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse},
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;

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

pub async fn serve_index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let template = IndexTemplate {
        servers: state.config.read().unwrap().servers.clone(),
    };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Template Error").into_response(),
    }
}

pub async fn serve_console(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let server = state.config.read().unwrap().servers.iter().find(|s| s.id == id).cloned();
    match server {
        Some(s) => match (ConsoleTemplate { server: s }).render() {
            Ok(html) => Html(html).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Template Error").into_response(),
        },
        None => Html("<h1>404 - Server Not Found</h1>".to_string()).into_response(),
    }
}

pub async fn ws_handler(Path(id): Path<String>, ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_client(socket, state, id))
}

async fn handle_client(mut browser_ws: WebSocket, state: Arc<AppState>, server_id: String) {
    let server_state = {
        let servers = state.servers.read().unwrap();
        match servers.get(&server_id) {
            Some(s) => s.clone(),
            None => return, // server doesn't exist
        }
    };

    {
        let history = server_state.history.read().await;
        for line in history.iter() {
            if browser_ws.send(AxumMessage::Text(line.clone().into())).await.is_err() {
                return;
            }
        }
    }

    let mut log_rx = server_state.log_tx.subscribe();
    let cmd_tx = server_state.cmd_tx.clone();

    let (mut browser_sender, mut browser_receiver) = browser_ws.split();

    let mut rx_task = tokio::spawn(async move {
        while let Ok(msg) = log_rx.recv().await {
            if browser_sender.send(AxumMessage::Text(msg.into())).await.is_err() { break; }
        }
    });

    let mut tx_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = browser_receiver.next().await {
            if let AxumMessage::Text(text) = msg {
                let _ = cmd_tx.send(text.to_string()).await;
            }
        }
    });

    tokio::select! {
        _ = (&mut rx_task) => tx_task.abort(),
        _ = (&mut tx_task) => rx_task.abort(),
    }
}