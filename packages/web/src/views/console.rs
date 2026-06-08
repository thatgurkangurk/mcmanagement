use dioxus::prelude::*;
use ui::components::Console as ConsoleComp;

#[component]
pub fn Console(id: String) -> Element {
    rsx! {
        ConsoleComp { id }
    }
}
