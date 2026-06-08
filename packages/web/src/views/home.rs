use dioxus::prelude::*;
use ui::components::ServerList;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "min-h-screen bg-neutral-950 text-neutral-200 font-sans antialiased selection:bg-emerald-500/20 selection:text-emerald-300 py-12 px-4 sm:px-6 lg:px-8",
            div { class: "max-w-3xl mx-auto space-y-8",

                div { class: "border-b border-neutral-800 pb-4 flex items-baseline justify-between",
                    h1 { class: "text-xs font-mono uppercase tracking-widest text-neutral-500",
                        "select a server"
                    }
                }

                div { class: "divide-y divide-neutral-900", ServerList {} }
            }
        }
    }
}