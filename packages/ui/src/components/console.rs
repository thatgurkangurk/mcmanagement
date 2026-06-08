use dioxus::prelude::*;
use std::collections::VecDeque;

const MAX_CONSOLE_LINES: usize = 40;

#[component]
pub fn Console(id: String) -> Element {
    #[allow(unused_mut)]
    let mut log_lines = use_signal(|| {
        let mut q = VecDeque::with_capacity(MAX_CONSOLE_LINES + 1);
        q.push_back(format!("connecting to worker [{}]...", id));
        q
    });
    let mut input_buffer = use_signal(String::new);
    #[allow(unused_mut)]
    let mut is_connected = use_signal(|| false);

    #[cfg(target_arch = "wasm32")]
    let target_id = id.clone();

    use_effect(move || {
        let _ = log_lines.read();
        
        #[cfg(target_arch = "wasm32")]
        {
            let _ = document::eval(
                "setTimeout(() => {
                    let el = document.getElementById('console-log-window');
                    if (el) el.scrollTop = el.scrollHeight;
                }, 0);"
            );
        }
    });

    #[allow(unused_mut, unused_variables)]
    let ws_task = use_coroutine(move |mut rx: UnboundedReceiver<String>| {
        #[cfg(target_arch = "wasm32")]
        let value = target_id.clone();
        
        async move {
            #[cfg(target_arch = "wasm32")]
            {
                use futures_util::{SinkExt, StreamExt};
                use gloo_net::websocket::{futures::WebSocket, Message};
                use serde_json::json;

                let window = web_sys::window().unwrap();
                let location = window.location();
                let host = location.host().unwrap();
                let protocol = if location.protocol().unwrap() == "https:" { "wss:" } else { "ws:" };
                let ws_url = format!("{}//{}/ws/{}", protocol, host, value);

                let mut push_log = |text: &str| {
                    let mut logs = log_lines.write();
                    for line in text.lines() {
                        logs.push_back(line.to_string());
                        if logs.len() > MAX_CONSOLE_LINES {
                            logs.pop_front();
                        }
                    }
                };

                match WebSocket::open(&ws_url) {
                    Ok(ws) => {
                        is_connected.set(true);
                        push_log("[System] connected to server.");
                        
                        let (mut write, mut read) = ws.split();

                        let send_task = dioxus::prelude::spawn(async move {
                            while let Some(cmd) = rx.next().await {
                                let payload = json!({
                                    "type": "stdin",
                                    "data": cmd
                                });
                                let _ = write.send(Message::Text(payload.to_string())).await;
                            }
                        });

                        while let Some(Ok(Message::Text(txt))) = read.next().await {
                            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&txt) {
                                let msg_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                
                                match msg_type {
                                    "logHistory" => {
                                        if let Some(lines) = payload.get("lines").and_then(|v| v.as_array()) {
                                            for line_val in lines {
                                                if let Some(line_str) = line_val.as_str() {
                                                    push_log(line_str);
                                                }
                                            }
                                        }
                                    }
                                    "stdout" | "stderr" => {
                                        if let Some(data) = payload.get("data").and_then(|v| v.as_str()) {
                                            push_log(data);
                                        }
                                    }
                                    "authFailure" => {
                                        if let Some(reason) = payload.get("reason").and_then(|v| v.as_str()) {
                                            push_log(&format!("[AUTH FAILURE]: {}", reason));
                                        }
                                    }
                                    _ => {} 
                                }
                            } else {
                                push_log(&txt);
                            }
                        }

                        send_task.cancel();
                        is_connected.set(false);
                        push_log("[System] connection lost.");
                    }
                    Err(_) => {
                        push_log("[ERR]: failed to establish connection.");
                    }
                }
            }
        }
    });

    let mut handle_submit = move || {
        let cmd = input_buffer.read().trim().to_string();
        if !cmd.is_empty() {
            ws_task.send(cmd);
            input_buffer.set(String::new());
        }
    };

    rsx! {
        div { class: "min-h-screen bg-neutral-950 text-neutral-200 font-sans antialiased py-12 px-4 sm:px-6 lg:px-8 flex flex-col",
            div { class: "max-w-4xl w-full mx-auto space-y-6 flex-1 flex flex-col",

                // header
                div { class: "border-b border-neutral-800 pb-4 flex items-baseline justify-between",
                    div { class: "flex items-center gap-4",
                        Link {
                            to: "/",
                            class: "text-xs font-mono text-neutral-500 hover:text-emerald-500 transition-colors cursor-pointer",
                            "<- back"
                        }
                        span { class: "text-xs font-mono text-neutral-700", "/" }
                        h1 { class: "text-xs font-mono tracking-widest text-neutral-300",
                            "console // {id}"
                        }
                    }
                    div { class: "flex items-center gap-2 text-xs font-mono",
                        div {
                            class: format!(
                                "w-1.5 h-1.5 rounded-full {}",
                                if is_connected() {
                                    "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]"
                                } else {
                                    "bg-red-500"
                                },
                            ),
                        }
                        span { class: "text-neutral-500 text-[10px]",
                            if is_connected() {
                                "ONLINE"
                            } else {
                                "OFFLINE"
                            }
                        }
                    }
                }

                div {
                    id: "console-log-window",
                    class: "flex-1 min-h-[500px] max-h-[70vh] bg-neutral-900 border border-neutral-800 rounded-lg p-5 overflow-y-auto font-mono text-xs leading-relaxed select-text scrollbar-thin",

                    for line in log_lines.read().iter() {
                        div { class: "whitespace-pre-wrap break-all text-neutral-300 tracking-tight py-0.5",
                            "{line}"
                        }
                    }
                }

                div { class: "flex items-center gap-3 bg-neutral-900 border border-neutral-800 rounded-lg px-4 py-2 font-mono text-xs focus-within:border-emerald-900 transition-colors shrink-0",
                    span { class: "text-emerald-500 select-none font-bold", ">" }
                    input {
                        class: "flex-1 bg-transparent border-none outline-none text-neutral-100 placeholder-neutral-600 font-mono py-1 disabled:opacity-50",
                        placeholder: if is_connected() { "send commands" } else { "Connecting..." },
                        value: "{input_buffer}",
                        disabled: !is_connected(),
                        oninput: move |e| input_buffer.set(e.value()),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                handle_submit();
                            }
                        },
                    }
                }
            }
        }
    }
}