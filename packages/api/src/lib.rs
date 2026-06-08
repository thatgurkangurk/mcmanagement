//! This crate contains all shared fullstack server functions.
use dioxus::prelude::*;
use shared::ServerInfo;


#[post("/api/echo")]
/// Echo the user input on the server.
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}

#[server]
pub async fn get_servers() -> Result<Vec<ServerInfo>, ServerFnError> {
    use std::sync::Arc;
    use server::state::AppState;
    use dioxus::prelude::dioxus_fullstack::FullstackContext;
    
    let context = FullstackContext::current()
        .ok_or_else(|| ServerFnError::new("Server functions must run within an active server scope"))?;
    
    let app_state = context
        .extension::<Arc<AppState>>()
        .ok_or_else(|| ServerFnError::new("Failed to retrieve AppState from fullstack extension layers"))?;
    
    let config = app_state.config.read().unwrap();
    let server_list = config.servers
        .iter()
        .map(|s| ServerInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            url: s.url.clone(),
        })
        .collect();

    Ok(server_list)
}

#[server]
pub async fn send_command(server_id: String, command: String) -> Result<(), ServerFnError> {
    use std::sync::Arc;
    use server::state::AppState;
    use dioxus::prelude::dioxus_fullstack::FullstackContext;

    let context = FullstackContext::current()
        .ok_or_else(|| ServerFnError::new("Server functions must run within an active server scope"))?;
    
    let app_state = context
        .extension::<Arc<AppState>>()
        .ok_or_else(|| ServerFnError::new("Failed to retrieve AppState from fullstack extension layers"))?;
    
    let servers = app_state.servers.read().unwrap();
    if let Some(server_state) = servers.get(&server_id) {
        let _ = server_state.cmd_tx.send(command).await;
        Ok(())
    } else {
        Err(ServerFnError::new("Minecraft cluster node not found"))
    }
}