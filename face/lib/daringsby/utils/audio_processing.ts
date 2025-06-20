import decodeAudio from "npm:audio-decode";
import audioBufferToWav from "npm:audiobuffer-to-wav";
import { AudioContext } from "npm:web-audio-api";
import { v4 as uuidv4 } from "npm:uuid";
import { join as pathJoin } from "jsr:@std/path";
import { arrayBufferToBase64 } from "./buffer_transformations.ts";
import emojiRegex from "npm:emoji-regex";
import logger from "../core/logger.ts";

/**
 * Detects silence in an AudioBuffer.
 *
 * @param {AudioBuffer} audioBuffer - The audio buffer to analyze.
 * @param {number} silenceThreshold - The amplitude threshold to consider as silence (e.g., 0.01).
 * @param {number} minSilenceDuration - Minimum duration (in seconds) to classify as silence.
 * @returns {Promise<Array<[number, number]>>} - A promise that resolves to an array of [start, end] pairs for silent segments.
 */
export async function detectSilence(
  audioBuffer: AudioBuffer,
  silenceThreshold: number = 0.01,
  minSilenceDuration: number = 0.5,
): Promise<Array<[number, number]>> {
  const channelData = audioBuffer.getChannelData(0); // Use the first channel
  const sampleRate = audioBuffer.sampleRate;
  const minSilenceSamples = Math.floor(minSilenceDuration * sampleRate);

  let silenceStart = null;
  const silenceSegments: Array<[number, number]> = [];

  for (let i = 0; i < channelData.length; i++) {
    if (Math.abs(channelData[i]) < silenceThreshold) {
      if (silenceStart === null) {
        silenceStart = i;
      }
    } else if (silenceStart !== null) {
      const silenceEnd = i;
      if (silenceEnd - silenceStart >= minSilenceSamples) {
        silenceSegments.push([
          silenceStart / sampleRate,
          silenceEnd / sampleRate,
        ]);
      }
      silenceStart = null;
    }
  }

  // Handle trailing silence
  if (
    silenceStart !== null &&
    channelData.length - silenceStart >= minSilenceSamples
  ) {
    silenceSegments.push([
      silenceStart / sampleRate,
      channelData.length / sampleRate,
    ]);
  }

  return silenceSegments;
}

/**
 * Clips a segment of audio from an AudioBuffer from a given index to an end index.
 *
 * @param {AudioBuffer} audioBuffer - The audio buffer to clip.
 * @param {number} start - Start index in samples.
 * @param {number} end - End index in samples.
 * @returns {AudioBuffer} - The clipped AudioBuffer.
 */
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

/**
 * Splits an AudioBuffer into chunks based on detected silence.
 *
 * @param {AudioBuffer} audioBuffer - The audio buffer to split.
 * @param {number} silenceThreshold - The amplitude threshold to consider as silence (e.g., 0.01).
 * @param {number} minSilenceDuration - Minimum duration (in seconds) to classify as silence.
 * @returns {Promise<AudioBuffer[]>} - A promise that resolves to an array of AudioBuffer chunks.
 */
export async function splitBySilence(
  audioBuffer: AudioBuffer,
  silenceThreshold: number = 0.01,
  minSilenceDuration: number = 0.5,
): Promise<AudioBuffer[]> {
  const silenceSegments = await detectSilence(
    audioBuffer,
    silenceThreshold,
    minSilenceDuration,
  );

  const audioChunks = [];
  let lastEnd = 0;
  const sampleRate = audioBuffer.sampleRate;

  for (const [start, end] of silenceSegments) {
    if (lastEnd < start) {
      // Extract non-silent segment as a new AudioBuffer
      const chunk = clip(
        audioBuffer,
        Math.floor(lastEnd * sampleRate),
        Math.floor(start * sampleRate),
      );
      audioChunks.push(chunk);
    }
    lastEnd = end;
  }

  // Add the final segment if there's remaining audio after the last silence
  if (lastEnd < audioBuffer.duration) {
    const chunk = clip(
      audioBuffer,
      Math.floor(lastEnd * sampleRate),
      Math.floor(audioBuffer.duration * sampleRate),
    );
    audioChunks.push(chunk);
  }

  return audioChunks;
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
  logger.debug(`Decoded WebM in ${endedAt - startedAt}ms.`);
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
  speakerId = "p234", //"p287", //"p230",
  languageId = "",
): Promise<string> {
  const host = Deno.env.get("COQUI_URL") ?? "http://localhost:5002";
  // TODO: Trace this type bug...text is arriving here undefined
  const response = await fetch(
    `${host}/api/tts?text=${
      encodeURIComponent(
        (text || "").replace(/\*/g, "").replace(emojiRegex(), ""),
      )
    }&speaker_id=${speakerId}&language_id=${languageId}`,
  );

  const wavData = await response.arrayBuffer();
  return arrayBufferToBase64(wavData);
}
