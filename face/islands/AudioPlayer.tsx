import { useCallback, useEffect, useRef } from "preact/hooks";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import {
  isValidSayMessage,
  SayMessage,
} from "../lib/daringsby/network/messages/SayMessage.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import { logger } from "../lib/daringsby/core/logger.ts";

export type EchoFunction = () => void;

export default function AudioPlayer(
  { serverRef }: { serverRef: { current: SocketConnection | null } },
) {
  const activeAudioElement = useRef<HTMLAudioElement | null>(null);
  const serverInstanceRef = useRef<SocketConnection | null>(null);

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

          // Stop currently playing audio
          if (activeAudioElement.current) {
            activeAudioElement.current.pause();
            activeAudioElement.current.currentTime = 0;
            document.body.removeChild(activeAudioElement.current);
            URL.revokeObjectURL(activeAudioElement.current.src);
          }

          // Attach the audio element to the DOM
          document.body.appendChild(audio);
          activeAudioElement.current = audio;

          audio.onended = () => {
            logger.debug("Audio playback ended successfully");
            logger.debug("Calling echo function");
            echo();
            logger.debug("Echo function called");

            // Remove the audio element from the DOM
            document.body.removeChild(audio);
            URL.revokeObjectURL(audioUrl);
            activeAudioElement.current = null;

            resolve();
          };

          audio.onerror = (e) => {
            logger.error({ e }, "Audio playback failed");
            document.body.removeChild(audio);
            URL.revokeObjectURL(audioUrl);
            activeAudioElement.current = null;
            reject(e);
          };

          audio.play()
            .then(() => {
              logger.debug("Audio playback started successfully");
            })
            .catch((e) => {
              logger.error({ e }, "Error attempting to play audio");
              document.body.removeChild(audio);
              URL.revokeObjectURL(audioUrl);
              activeAudioElement.current = null;
              reject(e);
            });
        } catch (error) {
          logger.error({ error }, "Error preparing audio for playback");
          reject(error);
        }
      });
    },
    [],
  );

  const handleMessage = useCallback(
    (message: SayMessage) => {
      logger.debug(`handleMessage called with message at: ${message.at}`);
      playSound(message.data.audio, () => {
        serverRef.current?.send({
          type: MessageType.Echo,
          data: message.data.words,
        });
      }).catch((error) => {
        logger.error({ error }, "Error handling new audio message");
      });
    },
    [playSound, serverRef],
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
      if (activeAudioElement.current) {
        activeAudioElement.current.pause();
        document.body.removeChild(activeAudioElement.current);
        URL.revokeObjectURL(activeAudioElement.current.src);
        activeAudioElement.current = null;
      }
    };
  }, [handleMessage]);

  return null;
}
