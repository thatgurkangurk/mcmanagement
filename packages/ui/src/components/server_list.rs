use dioxus::prelude::*;
use shared::ServerInfo;

#[component]
pub fn ServerList() -> Element {
    let servers_resource = use_resource(move || async move {
        api::get_servers().await
    });

    rsx! {
        div { class: "max-w-4xl mx-auto p-6",
            match &*servers_resource.read_unchecked() {
                Some(Ok(servers)) => {
                    if servers.is_empty() {
                        rsx! {
                            div { class: "p-4 rounded-lg bg-neutral-800 text-neutral-400 text-center",
                                "no servers in servers.json."
                            }
                        }
                    } else {
                        rsx! {
                            div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                                for server in servers {
                                    ServerCard { server: server.clone() }
                                }
                            }
                        }
                    }
                }
                Some(Err(err)) => {
                    rsx! {
                        div { class: "p-4 rounded-lg bg-red-900/40 text-red-400 border border-red-800",
                            "failed to connect to worker: {err}"
                        }
                    }
                }
                None => {
                    rsx! {
                        div { class: "animate-pulse text-neutral-400 text-center py-8", "polling the worker..." }
                    }
                }
            }
        }
    }
}

#[component]
fn ServerCard(server: ServerInfo) -> Element {

    rsx! {
        div { class: "p-5 rounded-xl bg-neutral-800 border border-neutral-700 hover:border-emerald-500 transition-all duration-200 shadow-md flex flex-col justify-between",
            div {
                div { class: "flex items-center justify-between mb-2",
                    h3 { class: "text-lg font-bold text-emerald-400", "{server.name}" }
                }
                p { class: "text-xs font-mono text-neutral-400 bg-neutral-900/60 p-2 rounded border border-neutral-700 break-all",
                    "{server.url}"
                }
            }

            div { class: "mt-4 flex items-center justify-between text-xs text-neutral-500",
                span { class: "font-mono", "ID: {server.id}" }

                button { class: "px-3 py-1.5 rounded-md bg-emerald-600 hover:bg-emerald-500 text-white font-medium shadow-sm transition-colors",
                    "open console"
                }
            }
        }
    }
}