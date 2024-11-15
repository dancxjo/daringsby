import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface ThoughtMessage extends SocketMessage {
    type: MessageType.Think;
    data: string;
}

export function isValidThoughtMessage(
    message: SocketMessage,
): message is ThoughtMessage {
    return message.type === MessageType.Think &&
        typeof message.data === "string";
}
