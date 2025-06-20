import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface SeeMessage extends SocketMessage {
  type: MessageType.See;
  data: string;
  at: string;
}

export function isValidSeeMessage(m: SocketMessage): m is SeeMessage {
  return m.type === MessageType.See && typeof m.data === "string" &&
    "at" in m && !!m.at && typeof m.at === "string" &&
    new Date(m.at).toString() !== "Invalid Date";
}
