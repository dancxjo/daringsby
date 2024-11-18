import { SocketMessage } from "./SocketMessage.ts";

export interface MessageHandler<T extends SocketMessage> {
    (message: T): void; // Hold your hat, sister, beccause we're not awaiting you
}
