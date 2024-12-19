import { useEffect, useRef } from "preact/hooks";
import logger from "../lib/daringsby/core/logger.ts";

interface ContinuousAudioCaptureProps {
  onSample?: (audioBlob: Blob) => void;
}

export default function ContinuousAudioCapture(
  { onSample }: ContinuousAudioCaptureProps,
) {
  const audioRef = useRef<MediaStream | null>(null);
  const recorderRef = useRef<MediaRecorder | null>(null);

  useEffect(() => {
    logger.info(
      "ContinuousAudioCapture component mounted, requesting microphone access.",
    );

    const startAudioCapture = async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: true,
        });
        audioRef.current = stream;
        logger.info("Microphone access granted.");

        const recorder = new MediaRecorder(stream, { mimeType: "audio/webm" });

        recorder.ondataavailable = (event) => {
          if (onSample && event.data.size > 0) {
            logger.info("Audio chunk captured.");
            onSample(event.data);
          }
        };

        recorderRef.current = recorder;
        recorder.start(500); // Request data every 500 ms
        logger.info("Audio recording started.");
      } catch (error) {
        logger.error({ error }, "Error accessing the microphone.");
      }
    };

    startAudioCapture();

    return () => {
      logger.info("Cleaning up audio resources.");
      if (recorderRef.current) {
        recorderRef.current.stop();
      }
      if (audioRef.current) {
        audioRef.current.getTracks().forEach((track) => track.stop());
      }
    };
  }, [onSample]);

  return <div>Continuous audio capture active. Listening...</div>;
}
