import { Handlers } from "$fresh/server.ts";
import { psyche } from "../lib/daringsby/core/psyche.ts";

export const handler: Handlers = {
  GET(req, _ctx) {
    return psyche.handleWebSocketConnection(req);
  },
};
