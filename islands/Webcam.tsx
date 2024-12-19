import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import logger from "../lib/daringsby/core/logger.ts";

interface WebcamProps {
  onSnap?: (image: string) => void;
  interval?: number; // Interval in milliseconds
}

export default function Webcam({ onSnap, interval = 10000 }: WebcamProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const snapshot = useSignal<string | null>(null);

  useEffect(() => {
    logger.info({ data: null }, "Webcam component mounted, starting webcam.");

    const startWebcam = async () => {
      logger.info("Starting webcam stream.");
      if (videoRef.current && !videoRef.current.srcObject) {
        try {
          const stream = await navigator.mediaDevices.getUserMedia({
            video: true,
          });
          videoRef.current.srcObject = stream;
          logger.info("Webcam stream started successfully.");
        } catch (error) {
          logger.error({ error }, "Error accessing the webcam.");
        }
      }
    };

    startWebcam();

    return () => {
      logger.info({ data: null }, "Stopping webcam stream.");
      if (videoRef.current && videoRef.current.srcObject) {
        const stream = videoRef.current.srcObject as MediaStream;
        stream.getTracks().forEach((track) => {
          track.stop();
          logger.info({ data: track }, "Stopped a track in the webcam stream.");
        });
      }
    };
  }, []);

  useEffect(() => {
    logger.info("Setting up capture interval.");

    const captureInterval = setInterval(() => {
      logger.info("Tick.");
      if (videoRef.current && canvasRef.current) {
        const canvas = canvasRef.current;
        const context = canvas.getContext("2d");
        if (context) {
          logger.info(
            "Capturing frame from webcam to canvas.",
          );
          context.drawImage(
            videoRef.current,
            0,
            0,
            canvas.width,
            canvas.height,
          );

          const image = canvas.toDataURL("image/jpg");
          snapshot.value = image;
          logger.info("Captured image as data URL.");

          if (onSnap) {
            logger.info("Calling onSnap callback.");
            onSnap(image);
          }
        }
      }
    }, interval);

    return () => {
      logger.info("Clearing capture interval.");
      clearInterval(captureInterval);
    };
  }, [interval, onSnap]);

  return (
    <div>
      {/* Video Preview */}
      <video
        ref={videoRef}
        autoPlay
        playsInline
      />

      {/* Hidden Canvas */}
      <canvas
        ref={canvasRef}
        style={{ display: "none" }}
      >
      </canvas>
    </div>
  );
}
