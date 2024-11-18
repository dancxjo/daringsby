import { SocketConnection } from "./sockets/connection.ts";
import { BehaviorSubject, Subscription } from "npm:rxjs";
import { Message } from "npm:ollama";

export interface Session {
    connection: SocketConnection;
    conversation: BehaviorSubject<Message[]>;
    subscriptions: Subscription[];
}

export const sessions = new Map<WebSocket, Session>();

export function addSession(
    socket: WebSocket,
    connection: SocketConnection,
): Session {
    const session: Session = {
        connection,
        conversation: new BehaviorSubject<Message[]>([]),
        subscriptions: [],
    };
    sessions.set(socket, session);
    return session;
}

export function removeSession(socket: WebSocket) {
    const session = sessions.get(socket);
    if (session) {
        session.subscriptions.forEach((subscription) =>
            subscription.unsubscribe()
        );
        sessions.delete(socket);
    }
}
