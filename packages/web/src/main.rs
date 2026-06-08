use dioxus::prelude::*;

use views::{Home, Console};

mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},

    #[route("/console/:id")]
    Console { id: String },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);

    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        let base_router = dioxus::server::router(App);

        let fullstack_router = server::bootstrap_server_engine(base_router).await?;

        Ok(fullstack_router)
    });
}

#[component]
fn App() -> Element {
    // Build cool things ✌️

    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Stylesheet { href: asset!("/assets/tailwind.css") }

        Router::<Route> {}
    }
}

#[component]
fn WebNavbar() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
