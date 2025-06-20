use futures_util::StreamExt;
use gloo_net::websocket::{Message, futures::WebSocket};
use sycamore::futures::spawn_local_scoped;
use sycamore::prelude::*;

#[component]
fn App<G: Html>(cx: Scope) -> View<G> {
    let log = create_signal(cx, String::new());

    spawn_local_scoped(cx, async move {
        console_error_panic_hook::set_once();
        let mut ws = WebSocket::open("ws://localhost:3000/ws").unwrap();
        while let Some(Ok(Message::Text(text))) = ws.next().await {
            log.set(format!("{}\n{}", log.get(), text));
        }
    });

    view! { cx,
        div(class="log") {
            h2 { "WebSocket Log" }
            pre { (log.get()) }
        }
    }
}

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    sycamore::render(|cx| view! { cx, App() });
}
