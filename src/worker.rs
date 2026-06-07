use crate::config::ServerConfig;
use crate::state::ServerState;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::{Duration, sleep};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::protocol::{Message as TungsteniteMessage, Role},
};
use url::Url;

const HISTORY_MAX_LENGTH: usize = 500;

/// the main background loop for a server
/// it attempts to connect, and if it fails or disconnects, it waits 5 seconds and retries
pub async fn run_worker(
    state: Arc<ServerState>,
    mut cmd_rx: mpsc::Receiver<String>,
    mut config_rx: watch::Receiver<ServerConfig>,
) {
    loop {
        let config = config_rx.borrow().clone();
        println!("Worker [{}]: Attempting connection...", config.id);

        tokio::select! {
            res = connect_and_handle(&config, &state, &mut cmd_rx) => {
                if let Err(e) = res {
                    eprintln!("Worker [{}]: Connection error: {}. Retrying in 5s...", config.id, e);
                }

                // sleep, but wake up immediately if config changes or is deleted
                tokio::select! {
                    _ = sleep(Duration::from_secs(5)) => {}
                    res = config_rx.changed() => {
                        if res.is_err() { break; } // channel dropped, exit worker
                    }
                }
            }
            res = config_rx.changed() => {
                if res.is_err() {
                    println!("Worker [{}]: Config removed, shutting down worker.", config.id);
                    break;
                } else {
                    println!("Worker [{}]: Config updated! Reconnecting...", config.id);
                }
            }
        }
    }
}

/// handles the actual TCP connection, WebSocket upgrade, and message piping.
async fn connect_and_handle(
    config: &ServerConfig,
    state: &Arc<ServerState>,
    cmd_rx: &mut mpsc::Receiver<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parsed_url = Url::parse(&config.url)?;
    let host = parsed_url.host_str().unwrap_or("localhost");
    let port = parsed_url.port_or_known_default().unwrap_or(80);
    let path = if parsed_url.path().is_empty() {
        "/"
    } else {
        parsed_url.path()
    };
    let addr = format!("{}:{}", host, port);

    let mut stream = TcpStream::connect(&addr).await?;

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
        path, addr, config.password
    );

    stream.write_all(request.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        headers.push_str(&line);
        if line == "\r\n" {
            break;
        }
    }

    if !headers.starts_with("HTTP/1.1 101") {
        return Err(format!("Handshake rejected by server:\n{}", headers).into());
    }

    println!("Worker [{}]: Connected successfully!", config.id);

    let mc_ws = WebSocketStream::from_raw_socket(reader, Role::Client, None).await;
    let (mut mc_sender, mut mc_receiver) = mc_ws.split();

    loop {
        tokio::select! {
            // Event A: We received a message FROM the Minecraft server
            msg = mc_receiver.next() => {
                match msg {
                    Some(Ok(TungsteniteMessage::Text(text))) => {
                        let text_str = text.to_string();

                        // lock the history buffer and push the new line
                        let mut history = state.history.write().await;
                        if history.len() >= HISTORY_MAX_LENGTH {
                            history.pop_front();
                        }
                        history.push_back(text_str.clone());
                        drop(history); // release the lock explicitly

                        // broadcast to any active browsers (ignore errors if nobody is listening)
                        let _ = state.log_tx.send(text_str);
                    }
                    Some(Err(e)) => return Err(e.into()), // Connection dropped
                    None => return Err("Stream ended naturally".into()),
                    _ => {} // ignore binary/ping/pong frames
                }
            }

            // command from browser
            cmd = cmd_rx.recv() => {
                if let Some(cmd_payload) = cmd {
                    // expected format: {"type":"stdin","data":"list\n"}
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&cmd_payload) {

                        // Extract the raw command string for the UI
                        if let Some(raw_command) = json.get("data").and_then(|v| v.as_str()) {

                            let display_text = format!("> {}", raw_command);

                            let mut history = state.history.write().await;
                            if history.len() >= 500 {
                                history.pop_front();
                            }
                            history.push_back(display_text.clone());
                            drop(history);

                            // broadcast it
                            let _ = state.log_tx.send(display_text);

                            let ts_msg = TungsteniteMessage::Text(cmd_payload.into());
                            if let Err(e) = mc_sender.send(ts_msg).await {
                                eprintln!("Failed to send command to MC: {}", e);
                            }
                        }
                    } else {
                        eprintln!("Worker [{}]: Received invalid JSON command: {}", config.id, cmd_payload);
                    }
                } else {
                    return Err("Command channel closed unexpectedly".into());
                }
            }
        }
    }
}
