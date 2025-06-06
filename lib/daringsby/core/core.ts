import { LinguisticProcessor } from "./lingproc.ts";
import { Ollama } from "npm:ollama";
import logger from "./logger.ts";

const forebrain = new Ollama({
  host: Deno.env.get("OLLAMA_HOST") || "http://10.0.0.180:11434",
});
const offload = new Ollama({
  host: Deno.env.get("OLLAMA2_HOST") || "http://192.168.1.122:11434",
});

const slow = new Ollama({
  host: Deno.env.get("OLLAMA3_HOST") || "http://localhost:11434",
});

export const lm = new LinguisticProcessor([forebrain, offload, slow]);
