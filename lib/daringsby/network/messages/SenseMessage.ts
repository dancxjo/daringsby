import { Impression, isImpression } from "../../core/interfaces.ts";
import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface SenseMessage extends SocketMessage {
    type: MessageType.Sense;
    data: Impression;
}

export function isValidSenseMessage(
    message: SocketMessage,
): message is SenseMessage {
    return message.type === MessageType.Sense && "data" in message &&
        isImpression(message.data);
}
