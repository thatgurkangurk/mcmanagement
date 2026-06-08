use crate::state::AppState;
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, Extension},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;

/// Handles incoming upgrade requests for the console WebSocket stream.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    let server_exists = {
        let servers = state.servers.read().unwrap();
        servers.contains_key(&id)
    };

    if server_exists {
        ws.on_upgrade(move |socket| handle_socket(socket, id, state))
    } else {
        axum::response::Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .body(axum::body::Body::from("Minecraft server worker not found"))
            .unwrap()
    }
}

async fn handle_socket(socket: WebSocket, id: String, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

let (s_state, log_tx, cmd_tx) = {
        let servers = state.servers.read().unwrap();
        let s_state = match servers.get(&id) {
            Some(srv) => Arc::clone(srv), 
            None => return,
        };
        
        let log = s_state.log_tx.clone();
        let cmd = s_state.cmd_tx.clone();
        
        (s_state, log, cmd)
    };

    let mut initial_history = s_state.history.read().await.clone();

    while let Some(line) = initial_history.pop_front() {
        if ws_tx.send(Message::Text(line.into())).await.is_err() {
            return; 
        }
    }

    let mut log_rx = log_tx.subscribe();

    
    // pipe new incoming logs from the worker out to the frontend terminal
    let mut log_forward_task = tokio::spawn(async move {
        while let Ok(line) = log_rx.recv().await {
            if ws_tx.send(Message::Text(line.into())).await.is_err() {
                break;
            }
        }
    });

    // accept inbound string commands from the terminal input and pipe them to the worker
    let mut command_recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            if let Message::Text(cmd) = msg {
                if cmd_tx.send(cmd.to_string()).await.is_err() {
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = &mut log_forward_task => command_recv_task.abort(),
        _ = &mut command_recv_task => log_forward_task.abort(),
    }

    println!("WebSocket terminal connection closed for worker target [{}]", id);
}