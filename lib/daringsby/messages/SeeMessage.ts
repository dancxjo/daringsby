import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface SeeMessage extends SocketMessage {
    type: MessageType.See;
    data: string;
}

export function isValidSeeMessage(m: SocketMessage): m is SeeMessage {
    return m.type === MessageType.See && typeof m.data === "string";
}
