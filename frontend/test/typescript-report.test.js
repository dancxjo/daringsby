const assert = require("assert");
const fs = require("fs");
const { JSDOM } = require("jsdom");

const html = fs.readFileSync("frontend/dist/index.html", "utf8");
const script = fs.readFileSync("frontend/dist/app.js", "utf8");

class MockWebSocket {
  static OPEN = 1;
  static instances = [];

  constructor() {
    this.readyState = MockWebSocket.OPEN;
    this.listeners = {};
    MockWebSocket.instances.push(this);
  }

  addEventListener(name, cb) {
    this.listeners[name] = cb;
  }

  send() {}
}

const dom = new JSDOM(html, {
  url: "http://localhost:3000/",
  runScripts: "outside-only",
});

dom.window.WebSocket = MockWebSocket;
dom.window.navigator.mediaDevices = undefined;
dom.window.navigator.geolocation = undefined;
dom.window.hljs = {
  highlightElement(element) {
    element.dataset.highlighted = "yes";
  },
};
dom.window.console = console;
dom.window.eval(script);

const ws = MockWebSocket.instances[0];
ws.onmessage({
  data: JSON.stringify({
    type: "FullHistory",
    data: {
      system_prompt: "sys",
      history: [],
      report: null,
      typescript: {
        source: 'import { look } from "pete:will";\nlook()',
        timestamp: "2026-05-12T13:00:00Z",
        results: [{ command: "look", output: "Latest vision: desk" }],
      },
    },
  }),
});

const source = dom.window.document.getElementById("typescript-source-code");
const results = dom.window.document.getElementById("typescript-results");

assert(html.includes("cdnjs.cloudflare.com/ajax/libs/highlight.js"));
assert.strictEqual(source.className, "language-typescript");
assert.match(source.textContent, /import \{ look \}/);
assert.strictEqual(source.dataset.highlighted, "yes");
assert.match(results.textContent, /look: Latest vision: desk/);

console.log("typescript-report ok");
