import { LinguisticProcessor } from "./lingproc.ts";
import { Ollama } from "npm:ollama";
import logger from "./logger.ts";

const forebrain = new Ollama({
  host: Deno.env.get("OLLAMA_HOST") || "http://172.17.0.1:11434",
});
const offload = new Ollama({
  host: Deno.env.get("OLLAMA2_HOST") || "http://192.168.0.19:11434",
});

const slow = new Ollama({
  host: Deno.env.get("OLLAMA3_HOST") || "http://ideapad.lan:11434",
});

export const lm = new LinguisticProcessor([forebrain, offload]);
