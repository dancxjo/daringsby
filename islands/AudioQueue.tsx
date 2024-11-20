import { useEffect, useRef } from "preact/hooks";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import {
    isValidSayMessage,
    SayMessage,
} from "../lib/daringsby/network/messages/SayMessage.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import { logger } from "../lib/daringsby/core/logger.ts";

export default function AudioPlayer(
    { serverRef }: { serverRef: { current: SocketConnection | null } },
) {
    const isPlaying = useRef(false);
    const listenerAttached = useRef(false);
    const audioRef = useRef<HTMLAudioElement | null>(null);

    const playSound = (wav: string) => {
        return new Promise<void>((resolve, reject) => {
            try {
                if (!audioRef.current) {
                    audioRef.current = new Audio();
                }

                const audio = audioRef.current;

                // Set the audio source
                const audioBlob = new Blob([
                    new Uint8Array(
                        atob(wav).split("").map((char) => char.charCodeAt(0)),
                    ),
                ], { type: "audio/wav" });
                const audioUrl = URL.createObjectURL(audioBlob);
                audio.src = audioUrl;

                audio.onended = () => {
                    logger.debug("Audio playback ended successfully");
                    isPlaying.current = false;
                    resolve();
                };

                audio.onerror = (e) => {
                    logger.error({ e }, "Audio playback failed");
                    isPlaying.current = false;
                    reject(e);
                };

                // Start playback
                isPlaying.current = true;
                audio.play().catch((e) => {
                    logger.error({ e }, "Error attempting to play audio");
                    isPlaying.current = false;
                    resolve(); // Resolve even if playback fails to continue processing
                });
            } catch (error) {
                logger.error({ error }, "Error preparing audio for playback");
                isPlaying.current = false;
                reject(error);
            }
        });
    };

    let lastWordsFrom = new Date();

    const handleMessage = async (message: SayMessage) => {
        const theseWordsFrom = new Date(message.at ?? new Date());
        if (theseWordsFrom < lastWordsFrom) {
            logger.debug(
                "Skipping obsolete message from the past:",
                message.at,
            );
            return;
        }
        lastWordsFrom = theseWordsFrom;
        logger.debug({
            message: message.at,
            data: message.data.words,
        }, "Received say message");

        if (isPlaying.current) {
            logger.debug(
                "Audio is currently playing, skipping message:",
                message.at,
            );
            return;
        }

        try {
            await playSound(message.data.wav);
            logger.debug("Finished playing message:", message.at);
            serverRef.current?.send({
                type: MessageType.Echo,
                data: message.data.words,
            });
        } catch (error) {
            logger.error({ error }, "Error playing sound");
        }
    };

    useEffect(() => {
        const server = serverRef.current;
        if (server && !listenerAttached.current) {
            server.onMessage(isValidSayMessage, MessageType.Say, handleMessage);
            listenerAttached.current = true;

            return () => {
                server.offMessage(MessageType.Say, handleMessage);
                listenerAttached.current = false;
            };
        }
    }, [serverRef.current]);

    return null;
}
