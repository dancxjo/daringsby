import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { MessageType } from "../lib/daringsby/messages/MessageType.ts";
import {
    isValidSayMessage,
    SayMessage,
} from "../lib/daringsby/messages/SayMessage.ts";
import { SocketConnection } from "../lib/daringsby/messages/SocketConnection.ts";
import { logger } from "../logger.ts";

export default function AudioQueue(
    { serverRef }: { serverRef: { current: SocketConnection | null } },
) {
    const isPlaying = useSignal(false);
    const isProcessingQueue = useSignal(false);
    const playqueue = useSignal<SayMessage[]>([]);

    const processQueue = async () => {
        if (isProcessingQueue.value) {
            return; // Prevent multiple overlapping calls
        }

        isProcessingQueue.value = true;

        while (playqueue.value.length > 0) {
            logger.info("Processing queue");
            const message = playqueue.value[0];
            if (!message) {
                break;
            }

            logger.info("Playing message");
            await playSound(message.data.wav);
            logger.info("Removing message from queue");
            playqueue.value = playqueue.value.slice(1);
        }

        isProcessingQueue.value = false;
    };

    const queueToPlay = (message: SayMessage) => {
        logger.info("Enqueuing message");
        playSound(message.data.wav).then(() => {
            serverRef.current?.send({
                type: MessageType.Echo,
                data: message.data.words,
            });
        });

        return;
        playqueue.value = [...playqueue.value, message];
        if (!isProcessingQueue.value) {
            processQueue(); // Trigger the queue processing if not already processing
        }
    };

    const playSound = async (wav: string) => {
        logger.info("Playing sound");
        if (isPlaying.value) {
            return false;
        }
        isPlaying.value = true;

        try {
            const audioBlob = new Blob([
                new Uint8Array(
                    atob(wav).split("").map((char) => char.charCodeAt(0)),
                ),
            ], { type: "audio/wav" });
            const audio = new Audio(URL.createObjectURL(audioBlob));
            await audio.play();
            return true;
        } catch (error) {
            logger.error("Error playing sound", error);
            return false;
        } finally {
            await new Promise((resolve) => setTimeout(resolve, 500)); // Ensure a slight delay before marking as not playing
            isPlaying.value = false;
        }

        return true;
    };

    useEffect(() => {
        const server = serverRef.current;
        if (server) {
            server.onMessage(
                isValidSayMessage,
                MessageType.Say,
                (message) => {
                    logger.info("Received say message");
                    queueToPlay(message);
                },
            );
        }
    }, [serverRef.current]);

    return null;
}
