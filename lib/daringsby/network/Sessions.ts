import { Voice } from "../core/voice.ts";
import { SocketConnection } from "./sockets/connection.ts";
import { Subscription } from "npm:rxjs";

export interface Session {
  connection: SocketConnection;
  subscriptions: Subscription[];
  voice: Voice;
}

export const sessions = new Map<WebSocket, Session>();

export function addSession(
  socket: WebSocket,
  connection: SocketConnection,
  voice: Voice,
): Session {
  const session: Session = {
    connection,
    subscriptions: [],
    voice,
  };
  sessions.set(socket, session);
  return session;
}

export function removeSession(socket: WebSocket) {
  const session = sessions.get(socket);
  if (session) {
    session.subscriptions.forEach((subscription) => subscription.unsubscribe());
    sessions.delete(socket);
  }
}
