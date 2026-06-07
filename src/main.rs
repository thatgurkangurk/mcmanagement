mod config;
mod state;
mod web;
mod worker;

use axum::{routing::get, Router};
use config::Config;
use notify::{RecursiveMode, Result as NotifyResult, Watcher};
use state::{AppState, ServerState};
use std::{collections::HashMap, fs, sync::{Arc, RwLock}};

#[tokio::main]
async fn main() {
    let config_path = option_env!("RUNNING_IN_DOCKER")
        .map(|_| "/app/servers.json")
        .unwrap_or("servers.json");

    let config_data = fs::read_to_string(config_path).expect("Failed to read config");
    let config: Config = serde_json::from_str(&config_data).expect("Failed to parse JSON");

    let mut server_states = HashMap::new();
    
    for server_cfg in &config.servers {
        let (s_state, cmd_rx) = ServerState::new(); 
        let s_state = Arc::new(s_state);
        
        server_states.insert(server_cfg.id.clone(), s_state.clone());
        
        tokio::spawn(worker::run_worker(server_cfg.clone(), s_state, cmd_rx));
    }

    let state = Arc::new(AppState {
        config: RwLock::new(config),
        servers: RwLock::new(server_states),
    });

    let watcher_state = state.clone();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                if let Ok(new_data) = fs::read_to_string(config_path) {
                    if let Ok(new_cfg) = serde_json::from_str::<Config>(&new_data) {
                        let mut cfg = watcher_state.config.write().unwrap();
                        *cfg = new_cfg;
                        println!("Configuration reloaded! (Workers require restart to apply changes)");
                    }
                }
            }
        }
    }).unwrap();
    watcher.watch(config_path.as_ref(), RecursiveMode::NonRecursive).unwrap();

    let app = Router::new()
        .route("/", get(web::serve_index))
        .route("/server/{id}", get(web::serve_console))
        .route("/ws/{id}", get(web::ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Manager listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}