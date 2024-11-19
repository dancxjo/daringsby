import { debounceTime, Observable, switchMap, toArray } from "npm:rxjs";
import logger from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { narrate } from "../utils/narration.ts";

export class Wit extends Genie<string> {
    constructor(
        name: string,
        description: string,
    ) {
        super(
            name,
            description,
            `These are the most recent experiences Pete has had. Narrate them in the first person. Do not embellish or invent new details; just go by the details here. If there are no sensations, simply say you feel nothing. Sensations: {{#sensations}}\nAt {{when}}, {{content.explanation}}\n{{/sensations}}`,
            narrate,
        );
        logger.info(`Wit: ${name} initialized`);
    }
}
