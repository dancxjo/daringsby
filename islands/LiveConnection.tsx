import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { IS_BROWSER } from "$fresh/runtime.ts";
import Geolocator from "./Geolocator.tsx";
import Webcam from "./Webcam.tsx";
import Mien from "./Mien.tsx";
import ThoughtBubble from "./ThoughtBubble.tsx";
import AudioQueue from "./AudioQueue.tsx";
import TextInput from "./TextInput.tsx";
import { logger } from "../logger.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import {
  initializeWebSocket,
  ws,
} from "../lib/daringsby/network/sockets/initializer.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import { isValidMienMessage } from "../lib/daringsby/network/messages/MienMessage.ts";
import { isValidSayMessage } from "../lib/daringsby/network/messages/SayMessage.ts";
import { isValidThoughtMessage } from "../lib/daringsby/network/messages/ThoughtMessage.ts";

import yml from "npm:yaml";
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

  const reportEvent = (event: Event) => {
    if (!serverRef.current) {
      logger.error("No server connection");
      return;
    }
    try {
      logger.debug("Reporting event to server");
      const deets = JSON.stringify(
        {
          ...event,
          type: event.type,
          code: (event as KeyboardEvent)?.code,
          // key: (event as KeyboardEvent)?.key,
          // keyCode: (event as KeyboardEvent)?.keyCode,
          // charCode: (event as KeyboardEvent)?.charCode,
          location: (event as KeyboardEvent)?.location,
          repeat: (event as KeyboardEvent)?.repeat,
          altKey: (event as KeyboardEvent)?.altKey,
          button: (event as MouseEvent)?.button,
          buttons: (event as MouseEvent)?.buttons,
          clientX: (event as MouseEvent)?.clientX,
          clientY: (event as MouseEvent)?.clientY,
          movementX: (event as MouseEvent)?.movementX,
          movementY: (event as MouseEvent)?.movementY,
          offsetX: (event as MouseEvent)?.offsetX,
          offsetY: (event as MouseEvent)?.offsetY,
          pageX: (event as MouseEvent)?.pageX,
          pageY: (event as MouseEvent)?.pageY,
          screenX: (event as MouseEvent)?.screenX,
          screenY: (event as MouseEvent)?.screenY,
          shiftKey: (event as KeyboardEvent)?.shiftKey,

          target: {
            id: (event.target as HTMLElement)?.id,
            classList: (event.target as HTMLElement)?.classList,
            nodeName: (event.target as HTMLElement)?.nodeName,
          },
          // target: event.target,
          timeStamp: event.timeStamp,
          // bubbles: event.bubbles,
          // cancelable: event.cancelable,
          // composed: event.composed,
          // defaultPrevented: event.defaultPrevented,
          // isTrusted: event.isTrusted,
        },
        null,
        2,
      );
      serverRef.current?.send({
        type: MessageType.Sense,
        data: {
          how: `I felt a ${event.type} event. Here are its details: ${deets}`,
          depth_low: 0,
          depth_high: 0,
          what: {
            when: new Date(), // This gets serialized as a string
            what: event,
          },
        },
      });
    } catch (error) {
      logger.error({ error }, "Failed to report event");
    }
  };

  const mien = useSignal("");
  const thought = useSignal("");
  const words = useSignal("");

  // addEventListener("keydown", reportEvent);
  // addEventListener("keyup", reportEvent);
  // addEventListener("keypress", reportEvent);
  // addEventListener("mousedown", reportEvent);
  // addEventListener("mouseup", reportEvent);
  // addEventListener("mousemove", reportEvent);

  return (
    <div class="container live-connection">
      <div class="row">
        <div class="col-12 col-md-6 mb-4 live-connection-inputs">
          <Webcam onSnap={sendSnapshot} interval={15000} />
          <TextInput onChange={sendText} />
          <Geolocator onChange={reportLocation} />
        </div>
        <div class="col-12 col-md-6 mb-4 live-connection-output">
          <Mien mien={mien} />
          <p class="spoken-words">{words.value}</p>
          <ThoughtBubble thought={thought} />
          <AudioQueue serverRef={serverRef} />
        </div>
      </div>
    </div>
  );
}
