import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import {
  isValidSayMessage,
  SayMessage,
} from "../lib/daringsby/network/messages/SayMessage.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import { logger } from "../lib/daringsby/core/logger.ts";

export type EchoFunction = () => void;

export default function AudioQueue(
  { serverRef }: {
    serverRef: { current: SocketConnection | null };
  },
) {
  const playqueue = useSignal<SayMessage[]>([]);
  let isProcessingQueue = false;

  const processQueue = async () => {
    if (isProcessingQueue) {
      return; // Prevent multiple overlapping calls
    }

    isProcessingQueue = true;

    while (playqueue.value.length > 0) {
      const message = playqueue.value.shift(); // Remove the first item from the queue
      if (!message) {
        break;
      }

      logger.debug("Playing message:", message.at);
      try {
        await playSound(message.data.wav, () => {
          serverRef.current?.send({
            type: MessageType.Echo,
            data: message.data.words,
          });
        });
        logger.debug("Finished playing message:", message.at);
      } catch (error) {
        logger.error({ error }, "Error playing sound");
      }
    }

    isProcessingQueue = false;

    // If new messages were added during processing, continue processing
    if (playqueue.value.length > 0) {
      processQueue();
    }
  };

  const queueToPlay = (message: SayMessage) => {
    logger.debug("Enqueuing message");
    playqueue.value = [...playqueue.value, message];

    // Only start processing if not already processing
    if (!isProcessingQueue) {
      processQueue();
    }
  };

  const playSound = (wav: string, echo: EchoFunction) => {
    return new Promise<void>((resolve, reject) => {
      try {
        const audioBlob = new Blob([
          new Uint8Array(
            atob(wav).split("").map((char) => char.charCodeAt(0)),
          ),
        ], { type: "audio/wav" });
        const audioUrl = URL.createObjectURL(audioBlob);
        const audio = new Audio(audioUrl);

        audio.onended = () => {
          logger.debug("Audio playback ended successfully");
          echo();
          resolve();
        };

        audio.onerror = (e) => {
          logger.error({ e }, "Audio playback failed");
          reject(e);
        };

        // Start playback
        audio.play().catch((e) => {
          logger.error({ e }, "Error attempting to play audio");
          reject(e);
        });
      } catch (error) {
        logger.error({ error }, "Error preparing audio for playback");
        reject(error);
      }
    });
  };

  useEffect(() => {
    const server = serverRef.current;
    if (server) {
      const handleMessage = (message: SayMessage) => {
        logger.debug({
          message: message.at,
          data: message.data.words,
        }, "Received say message");
        queueToPlay(message);
      };

      server.onMessage(isValidSayMessage, MessageType.Say, handleMessage);

      return () => {
        server.offMessage(MessageType.Say, handleMessage);
      };
    }
  }, [serverRef.current]);

  return null;
}
