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
dom.window.console = console;
dom.window.eval(script);

const ws = MockWebSocket.instances[0];
const conversationLog = dom.window.document.getElementById("conversation-log");
const systemPrompt = dom.window.document.getElementById("system-prompt");

function receive(type, data) {
  ws.onmessage({ data: JSON.stringify({ type, data }) });
}

receive("FullHistory", {
  system_prompt: "sys",
  history: [{ role: "user", content: "hello", timestamp: "2026-05-11T12:00:00Z" }],
  report: null,
});

assert.match(conversationLog.textContent, /user: hello/);

receive("FullHistory", {
  system_prompt: "new sys",
  history: [],
  report: null,
});

assert.match(conversationLog.textContent, /user: hello/, "empty history sync should not blank existing log");
assert.strictEqual(systemPrompt.textContent, "new sys");

receive("SystemPrompt", "newer sys");

assert.match(conversationLog.textContent, /user: hello/, "system prompt sync should not blank existing log");
assert.strictEqual(systemPrompt.textContent, "newer sys");

console.log("conversation-history-sync ok");
