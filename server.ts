import { serve } from "https://deno.land/std@0.200.0/http/server.ts";
import { HeartbeatSensor } from "./sensors/heartbeat.ts";
import { WebSocketSensor } from "./sensors/websocket.ts";
import { Psyche } from "./lib/Psyche.ts";
import { Ollama } from "npm:ollama";
import { OllamaChatter, OllamaInstructionFollower } from "./providers/ollama.ts";

const wsSensor = new WebSocketSensor();
const clients = new Set<WebSocket>();

const pete = new Psyche(
  [new HeartbeatSensor(), wsSensor],
  new OllamaInstructionFollower(new Ollama(), "gemma3"),
  new OllamaChatter(new Ollama(), "gemma3"),
  async (chunk: string) => {
    for (const ws of clients) {
      try {
        ws.send(chunk);
      } catch (_) {
        // ignore failed sends
      }
    }
  },
);

pete.run();

function page(): string {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Pete Chat</title>
  <script src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js" defer></script>
  <link href="https://cdn.jsdelivr.net/npm/tailwindcss@3.4.4/dist/tailwind.min.css" rel="stylesheet">
</head>
<body class="bg-gray-50" x-data="chat()">
  <div class="max-w-lg mx-auto p-4">
    <h1 class="text-2xl mb-4 text-center">Chat with Pete</h1>
    <div class="border h-64 overflow-y-auto p-2 mb-4 bg-white" id="log">
      <template x-for="line in lines" :key="line.id">
        <div class="mb-1" x-text="line.text"></div>
      </template>
    </div>
    <form @submit.prevent="send" class="flex gap-2">
      <input x-model="input" autofocus class="border p-2 flex-grow" placeholder="Say something" />
      <button type="submit" class="bg-blue-500 text-white px-4 py-2 rounded">Send</button>
    </form>
  </div>
<script>
function chat() {
  const ws = new WebSocket("ws://" + location.host + "/ws");
  return {
    lines: [],
    input: '',
    send() {
      ws.send(this.input);
      this.lines.push({ id: Date.now(), text: 'You: ' + this.input });
      this.input = '';
    }
  };
}
</script>
</body>
</html>`;
}

serve((req, info) => {
  const { pathname } = new URL(req.url);
  if (pathname === "/ws" && req.headers.get("upgrade") === "websocket") {
    const { socket, response } = Deno.upgradeWebSocket(req);
    const remote = (info.remoteAddr as Deno.NetAddr).hostname;
    clients.add(socket);
    wsSensor.connected(remote);
    socket.onclose = () => {
      clients.delete(socket);
      wsSensor.disconnected(remote);
    };
    socket.onmessage = (e) => {
      const text = String(e.data);
      wsSensor.received(remote, text);
      pete.conversation.push({ role: "user", content: text });
    };
    return response;
  }

  if (pathname === "/") {
    return new Response(page(), { headers: { "content-type": "text/html" } });
  }

  return new Response("Not found", { status: 404 });
});
