<<<<<<< HEAD
import { Stamped } from "../../core/interfaces.ts";
=======
>>>>>>> gapski
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
<<<<<<< HEAD

export type Base64EncodedImage = string;

export function stamp(m: SeeMessage): Stamped<Base64EncodedImage> {
  return {
    when: new Date(m.at),
    content: m.data,
  };
}
=======
>>>>>>> gapski
