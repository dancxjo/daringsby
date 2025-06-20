import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface TextMessage extends SocketMessage {
    type: MessageType.Text;
    data: string;
}

export function isValidTextMessage(
    message: SocketMessage,
): message is TextMessage {
    return message.type === MessageType.Text;
}
