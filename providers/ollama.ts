import { Ollama } from "npm:ollama";
import { InstructionFollower } from "../lib/InstructionFollower.ts";

export class OllamaInstructionFollower extends InstructionFollower {
    constructor(protected client: Ollama, protected model: string) {
        super();
    }

    async instruct(
        prompt: string,
        onChunk?: (chunk: string) => Promise<void>,
    ): Promise<string> {
        console.log(`Prompt: ${prompt}`);
        const stream = await this.client.generate({
            stream: true,
            model: this.model,
            prompt: prompt,
        });
        let response = "";
        for await (const chunk of stream) {
            const content = chunk.response || "";
            response += content;
            if (onChunk) {
                await onChunk(content);
            }
        }
        return response;
    }
}
