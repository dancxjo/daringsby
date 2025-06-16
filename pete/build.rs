use dioxus::prelude::*;
use dioxus_ssr::render_lazy;
use std::fs;
use std::path::PathBuf;

const SCRIPT: &str = r#"function chatApp() {
  return {
    ws: null,
    status: 'WS: connecting',
    log: [],
    input: '',
    audioQueue: [],
    audio: null,
    init() { this.connect(); },
    connect() {
      if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
        return;
      }
      if (this.ws) { this.ws.close(); }
      this.status = 'WS: connecting';
      this.ws = new WebSocket('ws://localhost:3000/ws');
      this.ws.onopen = () => this.status = 'WS: open';
      this.ws.onclose = () => { this.status = 'WS: closed'; setTimeout(() => this.connect(), 1000); };
      this.ws.onerror = () => this.status = 'WS: error';
      this.ws.onmessage = (ev) => {
        try {
          const data = JSON.parse(ev.data);
          if (data.text) {
            this.append('system', data.text);
          }
          if (data.audio) {
            this.audioQueue.push({ audio: data.audio, text: data.text || '' });
            this.playNext();
          }
        } catch (_) {
          this.append('system', ev.data);
        }
      };
    },
    playNext() {
      if (this.audio || this.audioQueue.length === 0) return;
      const { audio, text } = this.audioQueue.shift();
      this.audio = new Audio('data:audio/wav;base64,' + audio);
      this.audio.onended = () => {
        this.audio = null;
        this.ws.send(JSON.stringify({ type: 'played', text }));
        this.playNext();
      };
      this.audio.play();
    },
    append(role, text) {
      const el = this.$refs.log;
      const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 2;
      if (this.log.length && this.log[this.log.length - 1].role === role) {
        this.log[this.log.length - 1].text += text;
      } else {
        this.log.push({ role, text });
      }
      this.$nextTick(() => {
        if (atBottom) el.scrollTop = el.scrollHeight;
        if (role !== 'user') this.ws.send(JSON.stringify({ type: 'displayed', text }));
      });
    },
    send() {
      if (!this.input) return;
      this.append('user', this.input);
      this.ws.send(JSON.stringify({ type: 'user', message: this.input }));
      this.input = '';
    }
  }
}"#;

fn main() {
    let body = render_lazy(rsx! {
        div { "x-data": "chatApp()", "x-init": "init()", class: "section",
            div { id: "status", "x-text": "status", class: "mb-2 has-text-weight-bold" }
            div { id: "log", "x-ref": "log", class: "box flex flex-col space-y-1",
                template { "x-for": "msg in log", ":key": "msg.id",
                    div { ":class": "msg.role === 'user' ? 'has-text-info has-text-right' : 'has-text-left'", "x-text": "msg.text" }
                }
            }
            div { class: "field has-addons",
                div { class: "control is-expanded",
                    sl-input {
                      class: "input",
                      placeholder: "Say something...",
                      "x-model": "input",
                      style: "width: 100%;"
                    }
                }
                div { class: "control",
                    sl-button { r#type: "submit", variant: "primary", "@click.prevent": "send", "Send" }
                }
            }
        }
    });

    let page = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>Pete Console</title>\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css\">\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/@picocss/pico@1/css/pico.min.css\">\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.3.0/dist/themes/light.css\">\n  <style>#log.chat-log {{ white-space: pre-wrap; max-height: 60vh; overflow-y: auto; }}</style>\n  <script type=\"module\" src=\"https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.3.0/dist/shoelace.js\"></script>\n  <script src=\"https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js\" defer></script>\n</head>\n<body class=\"container\">\n  {body}\n  <script>{SCRIPT}</script>\n</body>\n</html>"
    );

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("index.html");
    fs::write(out, page).unwrap();
}
