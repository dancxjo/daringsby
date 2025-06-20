import { type Signal, signal, useSignal } from "@preact/signals";
import { MutableRef, useEffect, useRef } from "preact/hooks";
import { logger } from "../lib/daringsby/core/logger.ts";
import { HearMessage } from "../lib/daringsby/network/messages/HearMessage.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import { arrayBufferToBase64 } from "../lib/daringsby/utils/buffer_transformations.ts";

let currentMimeType = "";

export const mediaStream = signal<MediaStream | null>(null);

interface SpeechInputProps {
  // This will not be awaited
  onSample: (message: HearMessage) => void | Promise<void>;
}

export default function SpeechInput(props: SpeechInputProps) {
  const isListening = useSignal(true);

  const startListening = () => {
    logger.info("Starting microphone...");
    setupMicrophone(props, isListening);
  };

  const stopListening = () => {
    logger.info("Stopping microphone...");
  };

  useEffect(() => {
    handleListeningState(startListening, stopListening, isListening);
  }, [isListening.value]);

  return (
    <div>
      <label>
        <input
          type="checkbox"
          checked={isListening.value}
          onChange={() => {
            isListening.value = !isListening.value;
          }}
        />
        Listen
      </label>
    </div>
  );
}

function handleListeningState(
  startListening: () => void,
  stopListening: () => void,
  isListening: Signal<boolean>,
) {
  let cleanup = () => {};
  if (isListening.value) {
    cleanup = startListening() ?? cleanup;
  } else {
    cleanup = stopListening;
  }
  return cleanup;
}

function setupMicrophone(
  props: SpeechInputProps,
  isListening: Signal<boolean>,
) {
  let audioContext: AudioContext | null = null;
  let mediaStreamSource: MediaStreamAudioSourceNode | null = null;

  const mimeType = determineMimeType();
  if (!mimeType) {
    console.error("No supported MIME type found for MediaRecorder.");
    return () => {};
  }

  currentMimeType = mimeType;
  navigator.mediaDevices
    .getUserMedia({ audio: true })
    .then((stream) => {
      initializeMediaRecorder(
        stream,
        mimeType,
        props,
        isListening,
      );
    })
    .catch((error) => {
      console.error("Error accessing microphone:", error);
    });

  return () => cleanupMicrophone(audioContext, mediaStreamSource);
}

function determineMimeType(): string | null {
  if (MediaRecorder.isTypeSupported("audio/webm; codecs=opus")) {
    return "audio/webm; codecs=opus";
  } else if (MediaRecorder.isTypeSupported("audio/webm")) {
    return "audio/webm";
  } else if (MediaRecorder.isTypeSupported("audio/ogg; codecs=opus")) {
    return "audio/ogg; codecs=opus";
  }
  return null;
}

function initializeMediaRecorder(
  stream: MediaStream,
  mimeType: string,
  props: SpeechInputProps,
  isListening: Signal<boolean>,
) {
  const fragmentRecorder = new MediaRecorder(stream, { mimeType });
  mediaStream.value = stream;
  const audioContext = new AudioContext();

  fragmentRecorder.onstart = () => {
    logger.info("Fragment recorder started...");
  };

  fragmentRecorder.onstop = () => {
    logger.info("Fragment recorder stopped. Starting next fragment...");
    if (isListening.value) {
      recordNextFragment(fragmentRecorder, isListening);
    }
  };

  fragmentRecorder.ondataavailable = (event) => {
    handleDataAvailable(event, props);
  };

  recordNextFragment(fragmentRecorder, isListening);
}

function handleDataAvailable(
  event: BlobEvent,
  props: SpeechInputProps,
) {
  logger.info("ondataavailable event fired");

  if (event.data && event.data.size > 0) {
    logger.trace("Fragment data available, size:", event.data.size);
    processFragmentData(event.data, props);
  } else {
    logger.warn("No data available in ondataavailable");
  }
}

async function processFragmentData(
  data: Blob,
  props: SpeechInputProps,
) {
  try {
    const segData = await data.arrayBuffer();
    logger.info("onFragment is being called");
    await props.onSample({
      type: MessageType.Hear,
      data: {
        base64: arrayBufferToBase64(segData),
        mime: currentMimeType,
      },
      at: new Date().toISOString(),
    });
  } catch (error) {
    logger.error("Error during onFragment processing:", error);
  }
}

function recordNextFragment(
  fragmentRecorder: MediaRecorder,
  isListening: Signal<boolean>,
) {
  if (fragmentRecorder.state === "inactive" && isListening.value) {
    logger.info("Starting fragment recorder...");
    fragmentRecorder.start();
    setTimeout(() => {
      if (fragmentRecorder.state === "recording") {
        logger.info("Stopping fragment recorder...");
        fragmentRecorder.stop();
      }
    }, 500); // Record for 500ms
  }
}

function cleanupMicrophone(
  audioContext: AudioContext | null,
  mediaStreamSource: MediaStreamAudioSourceNode | null,
) {
  if (mediaStreamSource) {
    mediaStreamSource.disconnect();
    mediaStreamSource = null;
  }
  if (audioContext) {
    audioContext.close();
    audioContext = null;
  }
  if (mediaStream.value) {
    mediaStream.value.getTracks().forEach((track) => track.stop());
    mediaStream.value = null;
  }
}
