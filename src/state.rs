use crate::config::{Config, ServerConfig};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::sync::{RwLock as AsyncRwLock, broadcast, mpsc, watch};

pub struct ServerState {
    pub history: AsyncRwLock<VecDeque<String>>,
    pub log_tx: broadcast::Sender<String>,
    pub cmd_tx: mpsc::Sender<String>,
    pub config_tx: watch::Sender<ServerConfig>,
}

impl ServerState {
    pub fn new(
        initial_config: ServerConfig,
    ) -> (Self, mpsc::Receiver<String>, watch::Receiver<ServerConfig>) {
        let (log_tx, _) = broadcast::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        let (config_tx, config_rx) = watch::channel(initial_config);

        let state = Self {
            history: AsyncRwLock::new(VecDeque::with_capacity(500)),
            log_tx,
            cmd_tx,
            config_tx,
        };

        (state, cmd_rx, config_rx)
    }
}

pub struct AppState {
    pub config: RwLock<Config>,
    pub servers: RwLock<HashMap<String, Arc<ServerState>>>,
}
