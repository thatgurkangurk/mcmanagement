pub mod config;
pub mod state;
pub mod worker;
pub mod web_backend;

use axum::Router;
use config::Config;
use notify::{RecursiveMode, Result as NotifyResult, Watcher};
use state::{AppState, ServerState};
use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
};

pub async fn bootstrap_server_engine(
    router: Router,
) -> Result<Router, dioxus::prelude::ServerFnError> {
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
        if let Ok(event) = res
            && event.kind.is_modify()
        {
            let _ = notify_tx.send(());
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
            while notify_rx.try_recv().is_ok() {}

            if let Ok(new_data) = fs::read_to_string(config_path)
                && let Ok(new_cfg) = serde_json::from_str::<Config>(&new_data)
            {
                println!("detected servers.json modifications! synchronising workers...");

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
                        println!("starting new worker for [{}]", server_cfg.id);
                        let (s_state, cmd_rx, config_rx) = ServerState::new(server_cfg.clone());
                        let s_state = Arc::new(s_state);
                        servers.insert(server_cfg.id.clone(), s_state.clone());
                        tokio::spawn(worker::run_worker(s_state, cmd_rx, config_rx));
                    }
                }

                servers.retain(|id, _| {
                    let keep = current_ids.contains(id);
                    if !keep {
                        println!("terminating worker allocation for [{}]", id);
                    }
                    keep
                });
            }
        }
    });

    let finalised_router = router
    .route("/ws/{id}", axum::routing::get(web_backend::ws_handler))
    .layer(axum::Extension(state));

    Ok(finalised_router)
}
