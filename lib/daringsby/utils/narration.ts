import { Observable, of } from "npm:rxjs";
import { llamify } from "./llamification.ts";
import { sentenceBySentence, wholeResponse } from "./chunking.ts";

import { logger } from "../core/logger.ts";

let currentView: string;

export function see(view: string) {
  currentView = view;
}

// Phrase circumstances in the second person: You are ...
export function narrate(circumstances: string): Observable<string> {
  logger.info({ circumstances }, "Initializing Daringsby Narration");
  const prompt$ = of(
    `You are a linguistic processing unit, a constituent part of an artificial intelligence named Pete Daringsby, a pseudo-conscious experimental software application. You speak on behalf of Pete, in the first person, not as a simple LLM, but for the entity itself. Consider this a roleplaying narration that happens to coincide with real life (as measured by Pete's sensors and wits). Here are the circumstances for you to narrate appropriately. Circumstances & Intructions:\n${circumstances}\n\nNarrate the following events in the first person, as the character Pete Daringsby. (Pete's current view is attached as image data to assist you in your task. It is what Pete is looking at.)`,
  );
  return prompt$.pipe(
    llamify(Deno.env.get("OLLAMA_MODEL") || "gemma2:27b", {
      host: Deno.env.get("OLLAMA_URL") || "http://localhost:11434",
    }, {
      // images: currentView
      //   ? [currentView?.replace(/^data:image\/.*?;base64,/, "") ?? ""]
      //   : [],
    }),
    wholeResponse(),
  );
}
