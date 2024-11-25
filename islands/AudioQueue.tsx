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
  const playbackTimeout = useRef<number | null>(null);
  const recentMessages = useRef<Set<string>>(new Set());
  const MAX_HISTORY_SIZE = 10;

  const clearOldMessages = () => {
    if (recentMessages.current.size > MAX_HISTORY_SIZE) {
      const toDelete = [...recentMessages.current].slice(
        0,
        recentMessages.current.size - MAX_HISTORY_SIZE,
      );
      toDelete.forEach((item) => recentMessages.current.delete(item));
    }
  };

  const actuallyPlaySound = async (wav: string) => {
    try {
      if (!audioRef.current) {
        audioRef.current = new Audio();
      }

      const isValidWav = (base64) => {
        try {
          const decoded = atob(base64);
          logger.info("Decoded WAV:" + decoded.slice(0, 4));
          return decoded.slice(0, 4) === "RIFF";
        } catch {
          return false;
        }
      };

      if (!isValidWav(wav)) {
        throw new Error("Invalid WAV format");
      }

      const audioBlob = new Blob([
        new Uint8Array(
          atob(wav).split("").map((char) => char.charCodeAt(0)),
        ),
      ], { type: "audio/wav" });
      const audioUrl = URL.createObjectURL(audioBlob);

      const audio = audioRef.current;
      audio.src = audioUrl;

      const clearPlaybackTimeout = () => {
        if (playbackTimeout.current) {
          clearTimeout(playbackTimeout.current);
          playbackTimeout.current = null;
        }
      };

      audio.onended = () => {
        clearPlaybackTimeout();
        logger.debug("Audio playback ended successfully");
        isPlaying.current = false;
        processQueue();
      };

      audio.onerror = () => {
        clearPlaybackTimeout();
        logger.error("Audio playback failed");
        isPlaying.current = false;
        processQueue();
      };

      isPlaying.current = true;
      playbackTimeout.current = window.setTimeout(() => {
        logger.error("Audio playback timeout");
        isPlaying.current = false;
        processQueue();
      }, 10000);

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

    if (recentMessages.current.has(message.data.words)) {
      logger.debug("Skipping duplicate message");
      return;
    }

    recentMessages.current.add(message.data.words);
    clearOldMessages();

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
      listenerAttached.current = true;

      server.onMessage(isValidSayMessage, MessageType.Say, handleMessage);

      return () => {
        server.offMessage(MessageType.Say, handleMessage);
        listenerAttached.current = false;
      };
    }
  }, [serverRef, handleMessage]);

  return null;
}
