import { Sensation } from "../../core/Sensation.ts";
import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface SenseMessage extends SocketMessage {
  type: MessageType.Sense;
  data: Sensation;
}

export function isValidSenseMessage(
  message: SocketMessage,
): message is SenseMessage {
  return message.type === MessageType.Sense && "data" in message &&
    message.data !== null && typeof message.data === "object" &&
    "how" in message.data &&
    typeof message.data.how === "string" && "when" in message.data &&
    message.data.when !== null && message.data.when instanceof Date;
}
