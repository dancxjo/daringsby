import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface SayMessage extends SocketMessage {
  type: MessageType.Say;
  data: {
    words: string;
    wav: string; // base64 encoded
    style?: string; // emojis for a face
  };
}

export function isValidSayMessage(m: SocketMessage): m is SayMessage {
  return m.type === MessageType.Say;
}
