import { Ollama } from "npm:ollama";
import { OllamaClient } from "./Client.ts";
import { OllamaProcessor } from "./Processor.ts";
import {
    GenerateRequest,
    GenerateResponse,
    Method,
    Task,
} from "../../tasks.ts";
import { lastValueFrom } from "npm:rxjs";
import { stringify } from "../../chunking.ts";
import { sanitize, toEncodedWav } from "../../tts.ts";

(async () => {
    const ollama1 = new Ollama({
        host: "http://forebrain.lan:11434",
    });
    const ollama2 = new Ollama({
        host: "http://victus.lan:11434",
    });
    const ollama3 = new Ollama();
    const processor1 = new OllamaProcessor(new OllamaClient(ollama2));
    const processor2 = new OllamaProcessor(new OllamaClient(ollama1));
    const task: Task<GenerateRequest, GenerateResponse> = {
        method: Method.Generate,
        input: {
            prompt:
                "Tell me a story that includes lots of dates, middle initials, units of measure and other things that are tricky for a 2nd grader to read.",
        },
        abortController: new AbortController(),
    };

    const model = "llama3.2";
    const chunks = processor1.generate(task, model);

    chunks.pipe(stringify(), sanitize(processor2), toEncodedWav()).subscribe(
        (encodedWav) => {
            console.log(`Encoded`);
        },
    );
    await lastValueFrom(chunks);
})();
