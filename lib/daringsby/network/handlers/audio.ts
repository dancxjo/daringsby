import logger from "../../core/logger.ts";
import { HearMessage, isValidHearMessage } from "../messages/HearMessage.ts";
import { Session } from "../Sessions.ts";
import { psyche } from "../../core/psyche.ts";
import {
  decode,
  decodeWebm,
  join,
  toWav,
} from "../../utils/audio_processing.ts";
import { base64ToArrayBuffer } from "../../utils/buffer_transformations.ts";
import { splitBySilence } from "../../utils/audio_processing.ts";
import { getTranscription } from "../../utils/whisper.ts";

async function fromEncodedWebm(encodedWebM: string): Promise<AudioBuffer> {
  const webm = Uint8Array.from(atob(encodedWebM), (c) => c.charCodeAt(0));
  const buffer = await decodeWebm(webm);
  return buffer;
}

export function handleIncomingHearMessages(session: Session): void {
  logger.info("Setting up HearMessage handler for session");
  let mainBuffer: AudioBuffer | null = null;

  session.subscriptions.push(
    session.connection.incoming(isValidHearMessage).subscribe(
      async (message: HearMessage): Promise<void> => {
        logger.debug("Received a valid HearMessage");
        const audioBuffer = await fromEncodedWebm(message.data);
        logger.debug("Decoded audio buffer");

        if (!mainBuffer) {
          mainBuffer = audioBuffer;
        } else {
          mainBuffer = join(mainBuffer, audioBuffer);
        }

        // Split at silences until we have an array of at least two buffers
        const chunks = await splitBySilence(mainBuffer);
        if (chunks.length > 1) {
          for (let i = 0; i < chunks.length - 1; i++) {
            const wavData = await toWav(chunks[i]);
            // Send the chunk for transcription
            getTranscription(wavData, "", undefined).then(
              (transcription) => {
                logger.info({ transcription }, "Transcription received");
                // TODO: Enforce order of receipt here
                psyche.hear({
                  role: "user",
                  content: transcription.text,
                });
              },
            );
          }
          // Keep the last chunk as the new mainBuffer
          mainBuffer = chunks[chunks.length - 1];
        }
      },
    ),
  );
}

export default handleIncomingHearMessages;
