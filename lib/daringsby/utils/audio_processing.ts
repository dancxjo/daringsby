import decodeAudio from "npm:audio-decode";
import audioBufferToWav from "npm:audiobuffer-to-wav";
import { AudioContext } from "npm:web-audio-api";
import { v4 as uuidv4 } from "npm:uuid";
import { join as pathJoin } from "jsr:@std/path";
import { arrayBufferToBase64 } from "./buffer_transformations.ts";
import { logger } from "../core/logger.ts";

/** Detects if an audio buffer is mostly silent based on a given threshold */
export function detectSilence(
  audioBuffer: AudioBuffer,
  threshold: number = 0.003,
): boolean {
  const channelData = audioBuffer.getChannelData(0);
  for (let i = 0; i < channelData.length; i++) {
    if (Math.abs(channelData[i]) > threshold) {
      return false;
    }
  }
  return true;
}

/** Clips a segment of audio from an audiobuffer from a given index to an end index */
export function clip(
  audioBuffer: AudioBuffer,
  start: number,
  end: number,
): AudioBuffer {
  const context = new AudioContext();
  const duration = end - start;
  const outputBuffer = context.createBuffer(
    1,
    duration,
    audioBuffer.sampleRate,
  );
  const outputData = outputBuffer.getChannelData(0);
  const inputData = audioBuffer.getChannelData(0);
  for (let i = 0; i < duration; i++) {
    outputData[i] = inputData[start + i];
  }
  return outputBuffer;
}

export async function decodeWebm(webmData: ArrayBuffer): Promise<AudioBuffer> {
  const startedAt = Date.now();
  const tempDir = Deno.makeTempDirSync();
  const tempWebmPath = pathJoin(tempDir, `${uuidv4()}.webm`);
  const tempWavPath = pathJoin(tempDir, `${uuidv4()}.wav`);

  // Write WebM data to a temporary file
  await Deno.writeFile(tempWebmPath, new Uint8Array(webmData));

  const command = new Deno.Command("ffmpeg", {
    args: [
      "-i",
      tempWebmPath,
      "-f",
      "wav",
      "-ar",
      "16000", // Sample rate
      "-ac",
      "1", // Number of channels
      tempWavPath,
    ],
    stderr: "piped",
  });

  const process = command.spawn();
  const { success, stderr } = await process.output();

  if (!success) {
    const errorMessage = new TextDecoder().decode(stderr);
    console.error("FFmpeg Error:", errorMessage);
    throw new Error(`Failed to convert WebM to WAV: ${errorMessage}`);
  }

  // Read WAV data from the temporary file
  const wavData = await Deno.readFile(tempWavPath);

  // Clean up temporary files
  await Deno.remove(tempWebmPath);
  await Deno.remove(tempWavPath);

  const audioBuffer = await decodeAudio(wavData.buffer);
  const endedAt = Date.now();
  console.log(`Decoded WebM in ${endedAt - startedAt}ms.`);
  return audioBuffer;
}

export async function decode(audioData: Uint8Array): Promise<AudioBuffer> {
  const audioBuffer = await decodeAudio(audioData.buffer);
  return audioBuffer;
}

export function join(
  ...otherBuffers: AudioBuffer[]
): AudioBuffer {
  const buffer1 = otherBuffers.shift();
  if (!buffer1) {
    throw new Error("No audio buffers to join");
  }
  const buffer2 = otherBuffers.shift();
  if (!buffer2) {
    return buffer1;
  }
  if (otherBuffers.length > 0) {
    const firstJoin = join(buffer1, buffer2);
    return join(firstJoin, ...otherBuffers);
  }
  const context = new AudioContext();
  const outputBuffer = context.createBuffer(
    1,
    buffer1.length + buffer2.length,
    buffer1.sampleRate,
  );
  const outputData = outputBuffer.getChannelData(0);
  const buffer1Data = buffer1.getChannelData(0);
  const buffer2Data = buffer2.getChannelData(0);
  outputData.set(buffer1Data);
  outputData.set(buffer2Data, buffer1.length);
  return outputBuffer;
}

export function toWav(audioBuffer: AudioBuffer): Uint8Array {
  const wavData = audioBufferToWav(audioBuffer);
  return new Uint8Array(wavData);
}

export async function speak(
  text: string,
  speakerId_: undefined | string = undefined, //"Kumar Dahl", //"p230",
  languageId = "",
): Promise<string> {
  const speakerId = speakerId_ ?? Deno.env.get("SPEAKER") ?? "Kumar Dahl";
  const host = Deno.env.get("COQUI_URL") ?? "http://localhost:5002";
  const url = `${host}/api/tts?text=${
    encodeURIComponent(text)
  }&speaker_id=${speakerId}&language_id=${languageId}`;
  logger.info({ url }, "Speaking text");
  const response = await fetch(
    url,
  );

  const wavData = await response.arrayBuffer();
  return arrayBufferToBase64(wavData);
}
