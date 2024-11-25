import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { IS_BROWSER } from "$fresh/runtime.ts";
import Geolocator from "./Geolocator.tsx";
import Webcam from "./Webcam.tsx";
import Mien from "./Mien.tsx";
import ThoughtBubble from "./ThoughtBubble.tsx";
import AudioQueue from "./AudioQueue.tsx";
import TextInput from "./TextInput.tsx";
import { logger } from "../lib/daringsby/core/logger.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import {
  initializeWebSocket,
  ws,
} from "../lib/daringsby/network/sockets/initializer.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import { isValidMienMessage } from "../lib/daringsby/network/messages/MienMessage.ts";
import { isValidSayMessage } from "../lib/daringsby/network/messages/SayMessage.ts";
import { isValidThoughtMessage } from "../lib/daringsby/network/messages/ThoughtMessage.ts";

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
          if (message.data.style) mien.value = message.data.style;
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
    <div class="container live-connection">
      <div class="row">
        <div class="col-12 col-md-6 mb-4 live-connection-output">
          <Mien mien={mien} />
          <p class="spoken-words">{words.value}</p>
          <TextInput onChange={sendText} />
          <ThoughtBubble thought={thought} />
          <AudioQueue serverRef={serverRef} />
        </div>
        <div class="col-12 col-md-6 mb-4 live-connection-inputs">
          <Webcam onSnap={sendSnapshot} interval={10000} />
          <Geolocator onChange={reportLocation} />
        </div>
      </div>
    </div>
  );
}
