import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

interface TextMesage extends SocketMessage {
    type: MessageType.Text;
    data: string;
}

export function isValidTextMessage(
    message: SocketMessage,
): message is TextMesage {
    return message.type === MessageType.Text;
}
