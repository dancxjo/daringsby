import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";

interface WebcamProps {
    onSnap?: (image: string) => void;
    interval?: number; // Interval in milliseconds
}

export default function Webcam({ onSnap, interval = 10000 }: WebcamProps) {
    const videoRef = useRef<HTMLVideoElement>(null);
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const snapshot = useSignal<string | null>(null);

    useEffect(() => {
        // Start the webcam stream when the component mounts
        const startWebcam = async () => {
            if (videoRef.current) {
                try {
                    const stream = await navigator.mediaDevices.getUserMedia({
                        video: true,
                    });
                    videoRef.current.srcObject = stream;
                } catch (error) {
                    console.error("Error accessing the webcam:", error);
                }
            }
        };

        startWebcam();

        // Stop the webcam stream when the component unmounts
        return () => {
            if (videoRef.current && videoRef.current.srcObject) {
                const stream = videoRef.current.srcObject as MediaStream;
                stream.getTracks().forEach((track) => track.stop());
            }
        };
    }, []);

    useEffect(() => {
        // Capture snapshots at specified intervals
        const captureInterval = setInterval(() => {
            if (videoRef.current && canvasRef.current) {
                const canvas = canvasRef.current;
                const context = canvas.getContext("2d");
                if (context) {
                    context.drawImage(
                        videoRef.current,
                        0,
                        0,
                        canvas.width,
                        canvas.height,
                    );

                    // Convert the canvas content to a data URL and trigger the callback
                    const image = canvas.toDataURL("image/jpg");
                    snapshot.value = image; // Update signal with the captured image
                    if (onSnap) onSnap(image); // Call the onSnap callback if provided
                }
            }
        }, interval);

        // Clear the interval when the component unmounts
        return () => clearInterval(captureInterval);
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
