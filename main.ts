import { HeartbeatSensor } from "./sensors/heartbeat.ts";
import { WebSocketSensor } from "./sensors/websocket.ts";
import { Psyche } from "./lib/Psyche.ts";
import { Ollama } from "npm:ollama";
import {
  OllamaChatter,
  OllamaInstructionFollower,
} from "./providers/ollama.ts";

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


const pageHtml = Deno.readTextFileSync(
  new URL("./index.html", import.meta.url),
);

Deno.serve({
  port: 8000,
  onListen: () => {
    console.log("Server running at http://localhost:8000/");
  },
  handler: (req, info) => {
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
        try {
          const { name, message } = JSON.parse(String(e.data));
          wsSensor.received(remote, name, message);
          pete.conversation.push({
            role: "user",
            content: `${name}: ${message}`,
          });
        } catch {
          const text = String(e.data);
          wsSensor.received(remote, "Unknown", text);
          pete.conversation.push({ role: "user", content: text });
        }
      };
      return response;
    }

    if (pathname === "/") {
      return new Response(pageHtml, { headers: { "content-type": "text/html" } });
    }

    return new Response("Not found", { status: 404 });
  },
});

pete.run();
