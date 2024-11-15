import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface EchoMessage extends SocketMessage {
    type: MessageType.Echo;
    data: string; // The words that were spoken
}

export function isValidEchoMessage(
    message: SocketMessage,
): message is EchoMessage {
    return message.type === MessageType.Echo;
}
