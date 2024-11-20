import { debounceTime, Observable, tap, toArray } from "npm:rxjs";
import logger from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { narrate } from "../utils/narration.ts";

export class Wits extends Genie<string> {
    constructor(
        name: string,
        description: string,
    ) {
        super(
            name,
            description,
            `These are the most recent experiences Pete has had. Narrate them in the first person. Do not embellish or invent new details; just go by the details here. If there are no sensations, simply say you feel nothing. Sensations: {{#sensations}}\nAt {{when}}, {{content.explanation}}\n{{/sensations}}\n\nBe succinct. Don't repeat yourself. Don't add new details. Just narrate what's here. Make sure to rearrange the sensations in chronological order in the description.`,
            narrate,
        );
        logger.debug(`Wit: ${name} initialized`);
    }

    // Wits are rercursive
    override consult(): Observable<string> {
        return super.consult().pipe(
            tap((narration) => {} // this.feel({
                //     when: new Date(),
                //     content: { explanation: narration, content: narration },
                // }
            ),
        );
    }
}
