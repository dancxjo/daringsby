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
import { sanitize } from "../../tts.ts";

(async () => {
    const ollama1 = new Ollama({
        host: "http://forebrain.local:11434",
    });
    const ollama2 = new Ollama({
        host: "http://victus.local:11434",
    });
    const ollama3 = new Ollama();
    const client = new OllamaClient(ollama1);
    const processor1 = new OllamaProcessor(client);
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

    chunks.pipe(stringify(), sanitize(processor2)).subscribe((clean) => {
        console.log(clean);
    });
    await lastValueFrom(chunks);
})();
