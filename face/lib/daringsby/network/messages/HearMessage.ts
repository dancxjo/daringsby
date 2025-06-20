import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface HearMessage extends SocketMessage {
  type: MessageType.Hear;
  data: {
    base64: string;
    mime: string;
  };
  at: string;
}

export function isValidHearMessage(m: SocketMessage): m is HearMessage {
  return m.type === MessageType.Hear &&
    typeof m.data === "object" &&
    typeof (m.data as { base64?: unknown }).base64 === "string" &&
    typeof (m.data as { mime?: unknown }).mime === "string" &&
    "at" in m && !!m.at && typeof m.at === "string" &&
    new Date(m.at).toString() !== "Invalid Date";
}
