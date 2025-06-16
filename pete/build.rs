use dioxus::prelude::*;
use dioxus_ssr::render_lazy;
use std::fs;
use std::path::PathBuf;

const SCRIPT: &str = r#"function chatApp() {
  return {
    ws: null,
    status: 'WS: connecting',
    log: '',
    input: '',
    init() { this.connect(); },
    connect() {
      this.status = 'WS: connecting';
      this.ws = new WebSocket('ws://localhost:3000/ws');
      this.ws.onopen = () => this.status = 'WS: open';
      this.ws.onclose = () => { this.status = 'WS: closed'; setTimeout(() => this.connect(), 1000); };
      this.ws.onerror = () => this.status = 'WS: error';
      this.ws.onmessage = (ev) => {
        try {
          const data = JSON.parse(ev.data);
          if (data.text) {
            this.log += data.text;
            this.$nextTick(() => {
              this.ws.send(JSON.stringify({ type: 'displayed', text: data.text }));
            });
          }
        } catch (_) {
          this.log += ev.data;
        }
      };
    },
    send() {
      if (!this.input) return;
      this.ws.send(JSON.stringify({ type: 'user', message: this.input }));
      this.input = '';
    }
  }
}"#;

fn main() {
    let body = render_lazy(rsx! {
        div { "x-data": "chatApp()", "x-init": "init()", class: "section",
            div { id: "status", "x-text": "status", class: "mb-2 has-text-weight-bold" }
            pre { id: "log", "x-text": "log", class: "box" }
            div { class: "field has-addons",
                div { class: "control is-expanded",
                    sl-input { class: "input", placeholder: "Say something...", "x-model": "input" }
                }
                div { class: "control",
                    sl-button { variant: "primary", "@click.prevent": "send", "Send" }
                }
            }
        }
    });

    let page = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>Pete Console</title>\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css\">\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/@picocss/pico@1/css/pico.min.css\">\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.3.0/dist/themes/light.css\">\n  <style>#log {{ white-space: pre-wrap; }}</style>\n  <script type=\"module\" src=\"https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.3.0/dist/shoelace.js\"></script>\n  <script src=\"https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js\" defer></script>\n</head>\n<body class=\"container\">\n  {body}\n  <script>{SCRIPT}</script>\n</body>\n</html>"
    );

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("index.html");
    fs::write(out, page).unwrap();
}
