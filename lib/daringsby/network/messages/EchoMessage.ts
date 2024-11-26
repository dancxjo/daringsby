import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

export interface EchoMessage extends SocketMessage {
  type: MessageType.Echo;
  data: string; // The words that were spoken
  thought?: boolean; // If the words were spoken on the connection to self
}

export function isValidEchoMessage(
  message: SocketMessage,
): message is EchoMessage {
  return message.type === MessageType.Echo;
}
