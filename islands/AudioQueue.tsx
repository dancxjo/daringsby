import { useCallback, useEffect, useRef } from "preact/hooks";
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
  const queue = useRef<string[]>([]);

  const actuallyPlaySound = async (wav: string) => {
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
        processQueue();
      };

      audio.onerror = (e) => {
        logger.error({ e }, "Audio playback failed");
        isPlaying.current = false;
        processQueue();
      };

      // Start playback
      isPlaying.current = true;
      await audio.play();
    } catch (error) {
      logger.error({ error }, "Error preparing audio for playback");
      isPlaying.current = false;
      processQueue();
    }
  };

  const processQueue = () => {
    if (queue.current.length === 0) {
      return;
    }
    if (isPlaying.current) {
      return; // Don't start another sound if one is already playing
    }
    const next = queue.current.shift();
    if (next) {
      actuallyPlaySound(next);
    }
  };

  const playSound = (wav: string) => {
    return new Promise<void>((resolve, reject) => {
      queue.current.push(wav);
      if (!isPlaying.current) {
        processQueue();
      }
      resolve();
    });
  };

  let lastWordsFrom = useRef(new Date());
  let lastWordsSpoken = useRef("");

  const handleMessage = useCallback(async (message: SayMessage) => {
    const theseWordsFrom = new Date(message.at ?? new Date());
    if (theseWordsFrom < lastWordsFrom.current) {
      logger.debug(
        "Skipping obsolete message from the past:",
        message.at,
      );
      return;
    }
    lastWordsFrom.current = theseWordsFrom;
    logger.debug({
      message: message.at,
      data: message.data.words,
    }, "Received say message");

    if (message.data.words === lastWordsSpoken.current) {
      logger.debug("Skipping duplicate message");
      return;
    }
    try {
      lastWordsSpoken.current = message.data.words;
      await playSound(message.data.wav);
      logger.debug("Finished playing message:", message.at);
      serverRef.current?.send({
        type: MessageType.Echo,
        data: message.data.words,
      });
    } catch (error) {
      logger.error({ error }, "Error playing sound");
    }
  }, [serverRef]);

  useEffect(() => {
    const server = serverRef.current;
    if (server && !listenerAttached.current) {
      listenerAttached.current = true; // Mark as attached before attaching

      server.onMessage(isValidSayMessage, MessageType.Say, handleMessage);

      return () => {
        server.offMessage(MessageType.Say, handleMessage);
        listenerAttached.current = false; // Proper cleanup
      };
    }
  }, [serverRef, handleMessage]);

  return null;
}
