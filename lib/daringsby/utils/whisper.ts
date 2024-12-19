import { pino } from "npm:pino";

const logger = pino({ level: "debug" });

export interface TranscribedSegment {
  id: number;
  seek: number;
  start: number;
  end: number;
  text: string;
  tokens: number[];
  temperature: number;
  avg_logprob: number;
  compression_ratio: number;
  no_speech_prob: number;
  batchRecordedAt?: string;
}

export function isValidTranscribedSegment(
  data: unknown,
): data is TranscribedSegment {
  if (!data || typeof data !== "object") {
    return false;
  }

  const requiredFields = [
    "id",
    "seek",
    "start",
    "end",
    "text",
    "tokens",
    "temperature",
    "avg_logprob",
    "compression_ratio",
    "no_speech_prob",
  ];

  for (const field of requiredFields) {
    if (!(field in data)) {
      return false;
    }
  }
  return true;
}

export interface Transcription {
  text: string;
  segments: TranscribedSegment[];
  language: string;
}

export function isValidTranscription(data: unknown): data is Transcription {
  if (!data || typeof data !== "object") {
    return false;
  }
  if (!("text" in data) || typeof data.text !== "string") {
    return false;
  }
  if (!("segments" in data) || !Array.isArray(data.segments)) {
    return false;
  }
  if (!("language" in data) || typeof data.language !== "string") {
    return false;
  }

  if (data.segments.some((segment) => !isValidTranscribedSegment(segment))) {
    return false;
  }

  return true;
}

export async function getTranscription(
  wavData: Uint8Array,
  initialPrompt: string = "",
  signal?: AbortSignal,
): Promise<Transcription> {
  if (!(wavData instanceof Uint8Array) || wavData.length < 44) {
    throw new Error("Invalid WAV data provided for transcription.");
  }

  const whisperUrl = new URL(
    (Deno.env.get("WHISPER_URL") ?? "http://localhost:9000") + "/asr",
  );
  // whisperUrl.searchParams.append("language", "en");
  whisperUrl.searchParams.append("initial_prompt", initialPrompt);
  whisperUrl.searchParams.append("output", "json");
  const wavFile = new File([wavData], "audio.wav");

  const body = new FormData();
  body.append("audio_file", wavFile);

  logger.debug("Transcribing audio with initial prompt: %s", initialPrompt);
  const whisperResponse = await fetch(whisperUrl.toString(), {
    method: "POST",
    body: body,
    signal,
  });

  if (!whisperResponse.ok) {
    const errorText = await whisperResponse.text();
    logger.error(errorText, "Error transcribing audio");
    throw new Error(
      `Error transcribing audio: ${whisperResponse.statusText} - ${errorText}`,
    );
  }

  try {
    const obj = await whisperResponse.json();
    logger.debug(obj, "Received Whisper response");
    return obj;
  } catch (error) {
    logger.error(error, "Error parsing Whisper response");
    throw new Error("Error parsing Whisper response");
  }
}
