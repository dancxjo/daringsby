# Task 5: TTS Sentence Streamer

This document describes how to stream a sentence from the LLM and speak it using Coqui TTS.

## Overview

1. Stream text from `gemma3:27b` using the `llm` crate.
2. Detect the first complete sentence and extract any emoji.
3. Send the cleaned sentence to a Coqui TTS server.
4. Receive audio bytes and return them to the caller.

## Docker Compose

Run the Coqui TTS server locally using the following service definition:

```yaml
tts:
  image: ghcr.io/coqui-ai/tts
  ports:
    - "5002:5002"
  environment:
    - NVIDIA_VISIBLE_DEVICES=all
    - COQUI_TOS_AGREED=1
    - TTS_MODEL="tts_models/en/ljspeech/tacotron2-DDC"
    - VOCODER_MODEL="vocoder_models/en/ljspeech/hifigan_v1"
  command: ["TTS/server/server.py", "--model_name", "tts_models/en/vctk/vits"]
  runtime: nvidia
  volumes:
    - ./tts:/root/.local/share
    - /etc/timezone:/etc/timezone:ro
  deploy:
    resources:
      reservations:
        devices:
          - count: all
            capabilities: [gpu]
```

Set the following environment variables in `.env` or your configuration:

```dotenv
COQUI_URL=http://tts:5002
SPEAKER=Royston Min
```

Use this file when wiring TTS playback into the Voice Genie.
