mod config;
mod state;
mod web;
mod worker;

use axum::{Router, routing::get};
use config::Config;
use notify::{RecursiveMode, Result as NotifyResult, Watcher};
use state::{AppState, ServerState};
use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
};

#[tokio::main]
async fn main() {
    let config_path = option_env!("RUNNING_IN_DOCKER")
        .map(|_| "/app/servers.json")
        .unwrap_or("servers.json");

    let config_data = fs::read_to_string(config_path).expect("Failed to read config");
    let config: Config = serde_json::from_str(&config_data).expect("Failed to parse JSON");

    let mut server_states = HashMap::new();
    for server_cfg in config.servers.clone() {
        let (s_state, cmd_rx, config_rx) = ServerState::new(server_cfg.clone());
        let s_state = Arc::new(s_state);
        server_states.insert(server_cfg.id.clone(), s_state.clone());

        tokio::spawn(worker::run_worker(s_state, cmd_rx, config_rx));
    }

    let state = Arc::new(AppState {
        config: RwLock::new(config),
        servers: RwLock::new(server_states),
    });

    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                let _ = notify_tx.send(()); // signal the manager task
            }
        }
    })
    .unwrap();
    watcher
        .watch(config_path.as_ref(), RecursiveMode::NonRecursive)
        .unwrap();

    let manager_state = state.clone();
    tokio::spawn(async move {
        while let Some(_) = notify_rx.recv().await {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            while let Ok(_) = notify_rx.try_recv() {}

            if let Ok(new_data) = fs::read_to_string(config_path) {
                if let Ok(new_cfg) = serde_json::from_str::<Config>(&new_data) {
                    println!("Detected config change! Synchronizing workers...");

                    {
                        let mut cfg = manager_state.config.write().unwrap();
                        *cfg = new_cfg.clone();
                    }

                    let mut servers = manager_state.servers.write().unwrap();
                    let mut current_ids = std::collections::HashSet::new();

                    for server_cfg in new_cfg.servers {
                        current_ids.insert(server_cfg.id.clone());

                        if let Some(existing_state) = servers.get(&server_cfg.id) {
                            let _ = existing_state.config_tx.send(server_cfg);
                        } else {
                            println!("Started new worker for [{}]", server_cfg.id);
                            let (s_state, cmd_rx, config_rx) = ServerState::new(server_cfg.clone());
                            let s_state = Arc::new(s_state);
                            servers.insert(server_cfg.id.clone(), s_state.clone());
                            tokio::spawn(worker::run_worker(s_state, cmd_rx, config_rx));
                        }
                    }

                    // remove deleted workers
                    servers.retain(|id, _| {
                        let keep = current_ids.contains(id);
                        if !keep {
                            println!("Removing worker for [{}]", id);
                            // by returning false, we drop the ServerState.
                            // this drops config_tx, which signals the worker task to cleanly exit
                        }
                        keep
                    });
                }
            }
        }
    });

    let app = Router::new()
        .route("/", get(web::serve_index))
        .route("/server/{id}", get(web::serve_console))
        .route("/ws/{id}", get(web::ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Manager listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}
