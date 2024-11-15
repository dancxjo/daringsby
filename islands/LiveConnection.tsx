import { initializeWebSocket, ws } from "../lib/daringsby/signals/ws.ts";
import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { IS_BROWSER } from "$fresh/runtime.ts";
import Geolocator from "./Geolocator.tsx";
import Webcam from "./Webcam.tsx";
import Mien from "./Mien.tsx";
import ThoughtBubble from "./ThoughtBubble.tsx";
import AudioQueue from "./AudioQueue.tsx";
import TextInput from "./TextInput.tsx";
import { MessageType } from "../lib/daringsby/messages/MessageType.ts";
import { isValidMienMessage } from "../lib/daringsby/messages/MienMessage.ts";
import { isValidSayMessage } from "../lib/daringsby/messages/SayMessage.ts";
import { SocketConnection } from "../lib/daringsby/messages/SocketConnection.ts";
import { isValidThoughtMessage } from "../lib/daringsby/messages/ThoughtMessage.ts";
import { logger } from "../logger.ts";

export default function LiveConnection() {
    if (IS_BROWSER) {
        initializeWebSocket();
    }

    let server: SocketConnection | null = null;
    const serverRef = useRef<SocketConnection | null>(server);

    useEffect(() => {
        if (ws.value) {
            server = new SocketConnection(ws.value);
            serverRef.current = server;
            server.onMessage(
                isValidMienMessage,
                MessageType.Emote,
                (message) => {
                    mien.value = message.data;
                },
            );

            server.onMessage(
                isValidSayMessage,
                MessageType.Say,
                (message) => {
                    words.value = message.data.words;
                },
            );

            server.onMessage(
                isValidThoughtMessage,
                MessageType.Think,
                (message) => {
                    thought.value = message.data;
                },
            );
        } else {
            if (server) {
                server.hangup();
            }
        }
    }, [ws.value]);

    const reportLocation = (
        location: { longitude: number; latitude: number },
    ) => {
        if (!serverRef.current) {
            logger.error("No server connection");
            return;
        }
        try {
            serverRef.current?.send({
                type: MessageType.Geolocate,
                data: location,
                at: new Date().toISOString(),
            });
        } catch (e) {
            logger.error(e);
        }
    };

    const sendSnapshot = (image: string) => {
        if (!serverRef.current) {
            logger.error("No server connection");
            return;
        }
        try {
            serverRef.current?.send({
                type: MessageType.See,
                data: image,
            });
        } catch (error) {
            logger.error(error);
        }
    };

    const sendText = (text: string) => {
        if (!text.trim()) {
            return;
        }
        logger.debug("Sending text");
        if (!serverRef.current) {
            logger.error("No server connection");
            return;
        }
        try {
            logger.debug("Sending text to server");
            serverRef.current?.send({
                type: MessageType.Text,
                data: text,
            });
        } catch (error) {
            logger.error(error);
        }
    };

    const mien = useSignal("");
    const thought = useSignal("");
    const words = useSignal("");

    return (
        <div class="flex flex-col md:flex-row gap-4 p-4">
            <div class="flex-1 bg-white shadow-md rounded-lg p-6">
                <Webcam onSnap={sendSnapshot} interval={5000} />
                <TextInput onChange={sendText} />
                <Geolocator onChange={reportLocation} />
            </div>
            <div class="flex-1 bg-white shadow-md rounded-lg p-6">
                <Mien mien={mien} />
                <p class="spoken-words mt-4 text-gray-700">{words.value}</p>
                <ThoughtBubble thought={thought} />
                <AudioQueue serverRef={serverRef} />
            </div>
        </div>
    );
}
