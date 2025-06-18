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
    playing: false,
    stream: null,
    init() { this.connect(); this.initCamera(); },
    initCamera() {
      navigator.mediaDevices.getUserMedia({ video: true }).then(s => {
        this.stream = s;
        const v = this.$refs.video;
        v.srcObject = s;
        v.play();
      }).catch(() => {});
    },
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
      if (this.playing || this.audioQueue.length === 0) return;
      const { audio, text } = this.audioQueue.shift();
      const player = this.$refs.player;
      let mime = 'audio/wav';
      const tryPlay = () => {
        player.src = `data:${mime};base64,${audio}`;
        const attempt = player.play();
        if (attempt !== undefined) {
          attempt.catch(err => {
            if (err.name === 'NotSupportedError' && mime === 'audio/wav') {
              mime = 'audio/mpeg';
              tryPlay();
            } else if (err.name === 'NotAllowedError') {
              const resume = () => {
                document.removeEventListener('click', resume);
                tryPlay();
              };
              document.addEventListener('click', resume);
            } else {
              console.error('Audio playback failed:', err);
            }
          });
        }
      };
      player.onended = () => {
        this.playing = false;
        this.ws.send(JSON.stringify({ type: 'played', text }));
        console.log('sent played ack:', text);
        this.playNext();
      };
      this.playing = true;
      tryPlay();
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
        if (role !== 'user') {
          this.ws.send(JSON.stringify({ type: 'displayed', text }));
          console.log('sent displayed ack:', text);
        }
      });
    },
    send() {
      if (!this.input) return;
      this.append('user', this.input);
      this.ws.send(JSON.stringify({ type: 'user', message: this.input }));
      this.input = '';
    },
    capture() {
      const v = this.$refs.video;
      const c = document.createElement('canvas');
      c.width = v.videoWidth;
      c.height = v.videoHeight;
      c.getContext('2d').drawImage(v, 0, 0);
      const base64 = c.toDataURL('image/png').split(',')[1];
      this.ws.send(JSON.stringify({ type: 'image', mime: 'image/png', base64 }));
    }
  }
}"#;

fn main() {
    let body = render_lazy(rsx! {
        div { "x-data": "chatApp()", "x-init": "init()", class: "columns is-gapless is-fullheight",
            aside { class: "column is-one-quarter p-4 has-background-grey-light",
                div { id: "status", "x-text": "status", class: "has-text-weight-bold is-size-7" }
                video { autoplay: true, playsinline: true, "x-ref": "video", class: "w-full max-h-40" }
            }
            main { class: "column is-flex is-flex-direction-column p-4",
                ul { id: "log", "x-ref": "log", class: "box is-flex is-flex-direction-column space-y-1 is-flex-grow-1 list-style-none",
                    template { "x-for": "(msg, i) in log", ":key": "i",
                        li { ":class": "msg.role", "x-text": "msg.text" }
                    }
                }
                div { class: "field has-addons mt-auto",
                    div { class: "control is-expanded",
                        sl-input {
                          class: "input",
                          placeholder: "Say something...",
                          "x-model": "input",
                          style: "width: 100%;",
                        }
                    }
                    div { class: "control",
                        sl-button { r#type: "submit", variant: "primary", "@click.prevent": "send", "Send" }
                    }
                    div { class: "control",
                        sl-button { r#type: "button", variant: "default", "@click.prevent": "capture", "Snap" }
                    }
                }
            }
        }
    });

    let page = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>Pete Console</title>\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css\">\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/@picocss/pico@1/css/pico.min.css\">\n  <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.3.0/dist/themes/light.css\">\n  <style>#log.chat-log {{ white-space: pre-wrap; overflow-y: auto; }} #log.chat-log li.user {{ align-self: flex-end; background-color: #bfdbfe; padding: 0.25rem 0.5rem; border-radius: 0.375rem; }} #log.chat-log li.system {{ align-self: flex-start; background-color: #e5e7eb; padding: 0.25rem 0.5rem; border-radius: 0.375rem; }}</style>\n  <script type=\"module\" src=\"https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.3.0/dist/shoelace.js\"></script>\n  <script src=\"https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js\" defer></script>\n</head>\n<body class=\"container\">\n  {body}\n  <script>{SCRIPT}</script>\n</body>\n</html>"
    );

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("index.html");
    fs::write(out, page).unwrap();
}
