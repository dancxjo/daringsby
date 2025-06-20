import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface MienMessage extends SocketMessage {
    type: MessageType.Emote;
    data: string; // The emojis that were spoken
}

export function isValidMienMessage(
    message: SocketMessage,
): message is MienMessage {
    return message.type === MessageType.Emote &&
        typeof message.data === "string";
}
