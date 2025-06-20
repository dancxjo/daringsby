import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface SayMessage extends SocketMessage {
  type: MessageType.Say;
  data: {
    words: string;
    audio: string; // base64 encoded
  };
}

export function isValidSayMessage(m: SocketMessage): m is SayMessage {
  return m.type === MessageType.Say;
}
