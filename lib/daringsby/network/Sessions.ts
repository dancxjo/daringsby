import { psyche } from "../core/psyche.ts";
import { SocketConnection } from "./sockets/connection.ts";
import { Subscription } from "npm:rxjs";

export interface Session {
  connection: SocketConnection;
  subscriptions: Subscription[];
}

export const sessions = new Map<WebSocket, Session>();

export function addSession(
  socket: WebSocket,
  connection: SocketConnection,
): Session {
  const session: Session = {
    connection,
    subscriptions: [],
  };
  sessions.set(socket, session);
  psyche.voice.attachConnection(session.connection);
  return session;
}

export function removeSession(socket: WebSocket) {
  const session = sessions.get(socket);
  if (session) {
    session.subscriptions.forEach((subscription) => subscription.unsubscribe());
    sessions.delete(socket);
  }
}
