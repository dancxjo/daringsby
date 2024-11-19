import { BehaviorSubject, Observable, Subject } from "npm:rxjs";
import { filter } from "npm:rxjs/operators";
import logger from "../../core/logger.ts";
import { MessageHandler } from "../messages/MessageHandler.ts";
import { MessageType } from "../messages/MessageType.ts";
import {
    isValidSocketMessage,
    SocketMessage,
} from "../messages/SocketMessage.ts";

export class SocketConnection {
    protected messageHandlers = new Map<
        MessageType,
        MessageHandler<SocketMessage>[]
    >();
    protected incomingMessages = new Subject<SocketMessage>();
    readonly incoming$ = this.incomingMessages.asObservable();

    protected connectionStatusSubject = new BehaviorSubject<string>(
        "Disconnected",
    );
    readonly connectionStatus$ = this.connectionStatusSubject.asObservable();

    protected socketInfoSubject = new BehaviorSubject({
        url: "",
        readyState: 0,
        protocols: "",
    });
    readonly socketInfo$ = this.socketInfoSubject.asObservable();

    constructor(protected ws: WebSocket) {
        this.setupWebSocket();
    }

    protected setupWebSocket() {
        logger.debug("Setting up WebSocket");
        this.ws.onopen = () => {
            this.handleOpen();
        };
        this.ws.onerror = (error) => {
            this.handleError(error);
        };
        this.ws.onclose = (event) => {
            this.handleClose(event);
        };
        this.ws.onmessage = (event) => {
            this.handleIncomingMessage(event);
        };
    }

    protected handleOpen() {
        this.connectionStatusSubject.next("Connected");
        this.socketInfoSubject.next({
            url: this.ws.url ?? "",
            readyState: this.ws.readyState ?? 0,
            protocols: this.ws.protocol ?? "",
        });
        logger.debug("WebSocket connection established");
    }

    protected handleError(error: Event) {
        this.connectionStatusSubject.next("Error");
        logger.error("WebSocket error", error);
    }

    readonly closings: ((event: CloseEvent) => void)[] = [];

    protected handleClose(event: CloseEvent) {
        this.closings.forEach((closing) => closing(event));
        this.connectionStatusSubject.next("Disconnected");
        logger.debug("WebSocket connection closed");
        logger.warn("WebSocket connection closed", event);
    }

    protected handleIncomingMessage(event: MessageEvent) {
        // logger.debug("WebSocket message received");
        try {
            const message = JSON.parse(event.data);
            if (!isValidSocketMessage(message)) {
                logger.error(message, "Invalid WebSocket message");
                return;
            }
            this.incomingMessages.next(message);
            const handlers = this.messageHandlers.get(message.type);
            if (handlers) {
                logger.debug(
                    `Handling message of type ${MessageType[message.type]}`,
                );
                handlers.forEach((handler) => handler(message));
            }
        } catch (err) {
            logger.error(err, "Error parsing WebSocket message:");
        }
    }

    onMessage<T extends SocketMessage>(
        isValid: (m: SocketMessage) => m is T,
        type: MessageType,
        handler: MessageHandler<T>,
    ) {
        logger.debug(
            `Registering handler for message type ${MessageType[type]}`,
        );
        const handlers = this.messageHandlers.get(type) || [];
        const wrapper = (message: SocketMessage) => {
            if (!isValid(message)) {
                logger.error("Invalid message received");
                return;
            }
            handler(message);
        };
        handlers.push(wrapper);
        this.messageHandlers.set(type, handlers);
    }

    offMessage<T extends SocketMessage>(
        type: MessageType,
        handler: MessageHandler<T>,
    ) {
        logger.debug(
            `Removing handler for message type ${MessageType[type]}`,
        );
        const handlers = this.messageHandlers.get(type) || [];
        // @ts-ignore: Today fix type specificity here
        const index = handlers.indexOf(handler);
        if (index !== -1) {
            handlers.splice(index, 1);
        }
        this.messageHandlers.set(type, handlers);
    }

    incoming<M extends SocketMessage>(
        validator: (msg: SocketMessage) => msg is M,
    ): Observable<M> {
        return this.incoming$.pipe(filter(validator));
    }

    send(message: SocketMessage) {
        logger.debug("Sending message through WebSocket");
        if (!message.at) {
            message.at = new Date().toISOString();
        }
        if (this.ws.readyState !== WebSocket.OPEN) {
            logger.error("WebSocket is not open");
            return;
        }
        this.ws.send(JSON.stringify(message));
    }

    get isOpen(): boolean {
        return this.ws.readyState === WebSocket.OPEN;
    }

    hangup() {
        this.ws.close();
    }
}
