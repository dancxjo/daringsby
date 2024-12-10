import { Sensation } from "../../core/newt.ts";
import { MessageType } from "./MessageType.ts";

export interface SocketMessage {
  type: MessageType;
  data: Blob | string | Record<string, unknown> | Sensation;
  at?: string;
}

export function isValidSocketMessage(
  message: unknown,
): message is SocketMessage {
  if (typeof message !== "object" || message === null) {
    return false;
  }
  if (!("type" in message) || !("data" in message)) {
    return false;
  }
  return true;
}
