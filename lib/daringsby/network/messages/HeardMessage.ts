import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface HeardMessage extends SocketMessage {
  type: MessageType.Heard;
  data: string;
}

export function isValidHeardMessage(m: SocketMessage): m is HeardMessage {
  return m.type === MessageType.Heard && typeof m.data === "string" &&
    typeof m.at === "string";
}
