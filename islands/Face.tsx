import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { IS_BROWSER } from "$fresh/runtime.ts";
import Geolocator from "./Geolocator.tsx";
import Webcam from "./Webcam.tsx";
import Mien from "./Mien.tsx";
import ThoughtBubble from "./ThoughtBubble.tsx";
import SpokenWords from "./SpokenWords.tsx";
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
import Body from "./Body.tsx";

export default function Face() {
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

  return (
    <div
      class="face"
      style={{
        position: "relative",
        width: "100vw",
        height: "100vh",
        overflow: "hidden",
      }}
    >
      <Webcam
        onSnap={sendSnapshot}
        interval={1000}
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          width: "100vw",
          height: "100vh",
          objectFit: "cover",
          zIndex: -1,
        }}
      />
      <div
        style={{
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          width: "100vw",
          height: "100vh",
          position: "absolute",
          zIndex: 1,
        }}
      >
        <Mien
          mien={mien}
          style={{
            textAlign: "center",
            fontSize: "calc(5vw + 5vh)", // Dynamically scale font size
            lineHeight: "1", // Use single line height for max font scaling
            color: "#fff", // Adjust color for readability
          }}
        />
      </div>
      <Geolocator onChange={reportLocation} />
      <AudioQueue serverRef={serverRef} />
    </div>
  );
}
