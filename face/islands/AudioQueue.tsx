import { useCallback, useEffect, useRef } from "preact/hooks";
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
  const playqueue = useRef<SayMessage[]>([]);
  const isProcessingQueue = useRef<boolean>(false);
  const serverInstanceRef = useRef<SocketConnection | null>(null);
  const activeAudioElements = useRef<Set<HTMLAudioElement>>(new Set());

  const playSound = useCallback(
    (wav: string, echo: EchoFunction) => {
      return new Promise<void>((resolve, reject) => {
        try {
          const audioBlob = new Blob(
            [
              new Uint8Array(
                atob(wav)
                  .split("")
                  .map((char) => char.charCodeAt(0)),
              ),
            ],
            { type: "audio/wav" },
          );
          const audioUrl = URL.createObjectURL(audioBlob);
          const audio = new Audio(audioUrl);

          // Attach the audio element to the DOM to prevent garbage collection
          document.body.appendChild(audio);

          // Add the audio to the set to keep a reference
          activeAudioElements.current.add(audio);

          audio.onended = () => {
            logger.debug("Audio playback ended successfully");
            logger.debug("Calling echo function");
            echo();
            logger.debug("Echo function called");

            // Remove the audio from the set and the DOM
            activeAudioElements.current.delete(audio);
            document.body.removeChild(audio);

            // Revoke the object URL to free memory
            URL.revokeObjectURL(audioUrl);

            resolve();
          };

          audio.onerror = (e) => {
            logger.error({ e }, "Audio playback failed");

            // Remove the audio from the set and the DOM
            activeAudioElements.current.delete(audio);
            document.body.removeChild(audio);

            // Revoke the object URL to free memory
            URL.revokeObjectURL(audioUrl);

            reject(e);
          };

          audio.play()
            .then(() => {
              logger.debug("Audio playback started successfully");
            })
            .catch((e) => {
              logger.error({ e }, "Error attempting to play audio");

              // Remove the audio from the set and the DOM
              activeAudioElements.current.delete(audio);
              document.body.removeChild(audio);

              // Revoke the object URL to free memory
              URL.revokeObjectURL(audioUrl);

              reject(e);
            });
        } catch (error) {
          logger.error({ error }, "Error preparing audio for playback");
          reject(error);
        }
      });
    },
    [], // Empty dependency array
  );

  const processQueue = useCallback(() => {
    if (isProcessingQueue.current) return;

    isProcessingQueue.current = true;

    (async () => {
      while (playqueue.current.length > 0) {
        const message = playqueue.current.shift();
        if (!message) continue;

        logger.debug(`Playing message at: ${message.at}`);
        try {
          await playSound(message.data.audio, () => {
            serverRef.current?.send({
              type: MessageType.Echo,
              data: message.data.words,
            });
          });
          logger.debug(`Finished playing message at: ${message.at}`);
        } catch (error) {
          logger.error({ error }, "Error playing sound");
        }
      }

      isProcessingQueue.current = false;
    })();
  }, []); // Empty dependency array

  const queueToPlay = useCallback(
    (message: SayMessage) => {
      logger.debug(`queueToPlay called with message at: ${message.at}`);
      playqueue.current.push(message);
      logger.debug(`Current playqueue size: ${playqueue.current.length}`);

      if (!isProcessingQueue.current) {
        processQueue();
      }
    },
    [], // Empty dependency array
  );

  const handleMessage = useCallback(
    (message: SayMessage) => {
      logger.debug(`handleMessage called with message at: ${message.at}`);
      queueToPlay(message);
    },
    [], // Empty dependency array
  );

  useEffect(() => {
    const server = serverRef.current;

    if (server) {
      if (!serverInstanceRef.current) {
        logger.debug("Adding listener to server instance");
        server.onMessage(isValidSayMessage, MessageType.Say, handleMessage);
        serverInstanceRef.current = server;
      } else if (serverInstanceRef.current !== server) {
        logger.debug("Server instance changed. Updating listener.");
        serverInstanceRef.current.offMessage(MessageType.Say, handleMessage);
        server.onMessage(isValidSayMessage, MessageType.Say, handleMessage);
        serverInstanceRef.current = server;
      }
    }

    // Cleanup function to remove listener when component unmounts
    return () => {
      if (serverInstanceRef.current) {
        logger.debug("Cleaning up listener from server instance");
        serverInstanceRef.current.offMessage(MessageType.Say, handleMessage);
        serverInstanceRef.current = null;
      }
    };
  }, [handleMessage]);

  return null;
}
