import { HeartbeatSensor } from "./sensors/heartbeat.ts";
import { WebSocketSensor } from "./sensors/websocket.ts";
import { Psyche } from "./lib/Psyche.ts";
import { Ollama } from "npm:ollama";

import {
  OllamaChatter,
  OllamaInstructionFollower,
} from "./providers/ollama.ts";
import "npm:dotenv/config";

const wsSensor = new WebSocketSensor();
const clients = new Set<WebSocket>();

const pete = new Psyche(
  [
    new HeartbeatSensor(),
    wsSensor,
  ],
  new OllamaInstructionFollower(
    new Ollama({ host: Deno.env.get("OLLAMA_URL") }),
    "gemma3:27b",
  ),
  new OllamaChatter(new Ollama(), "gemma3"),
  {
    onPrompt: async (prompt: string) => {
      const payload = JSON.stringify({ type: "pete-prompt", text: prompt });
      for (const ws of clients) {
        try {
          ws.send(payload);
        } catch (_) {
          // ignore failed sends
        }
      }
    },
    onSay: async (text: string) => {
      Deno.stdout.writeSync(
        new TextEncoder().encode(`>`),
      );
      const payload = JSON.stringify({ type: "pete-says", text });
      for (const ws of clients) {
        try {
          ws.send(payload);
        } catch (_) {
          // ignore failed sends
        }
      }
    },
    onStream: async (chunk: string) => {
      const payload = JSON.stringify({ type: "pete-stream", text: chunk });
      for (const ws of clients) {
        try {
          ws.send(payload);
        } catch (_) {
          // ignore failed sends
        }
      }
    },
    wsSensor,
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
          const data = JSON.parse(String(e.data));
          if (data.type === "echo") {
            wsSensor.echoed(remote, data.message);
            pete.confirm_echo(data.message);
            return;
          }
          const { name, message } = data;
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
      return new Response(pageHtml, {
        headers: { "content-type": "text/html" },
      });
    }

    return new Response("Not found", { status: 404 });
  },
});

pete.run();
