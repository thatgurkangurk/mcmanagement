use crate::config::Config;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc, RwLock as AsyncRwLock};

pub struct ServerState {
    pub history: AsyncRwLock<VecDeque<String>>,
    pub log_tx: broadcast::Sender<String>,
    pub cmd_tx: mpsc::Sender<String>,
}

impl ServerState {
    pub fn new() -> (Self, mpsc::Receiver<String>) {
        let (log_tx, _) = broadcast::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        
        let state = Self {
            history: AsyncRwLock::new(VecDeque::with_capacity(500)),
            log_tx,
            cmd_tx,
        };
        
        (state, cmd_rx)
    }
}

pub struct AppState {
    pub config: RwLock<Config>,
    pub servers: RwLock<HashMap<String, Arc<ServerState>>>,
}