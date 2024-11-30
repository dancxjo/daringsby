import { LinguisticProcessor } from "./linguisticProcessor.ts";
import { Ollama } from "npm:ollama";

const forebrain = new Ollama({
  host: Deno.env.get("OLLAMA_HOST") || "http://forebrain.lan:11434",
});
const offload = new Ollama({
  host: Deno.env.get("OLLAMA2_HOST") || "http://victus.lan:11435",
});

const slow = new Ollama({
  host: Deno.env.get("OLLAMA3_HOST") || "http://ideapad.lan:11436",
});

const processor = new LinguisticProcessor([ollama]);
