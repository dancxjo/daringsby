import { logger } from "../core/logger.ts";
import {
    catchError,
    Observable,
    of,
    ReplaySubject,
    switchMap,
    tap,
    throttleTime,
    toArray,
} from "npm:rxjs";
import { Faculty, Sensation, Sensitive } from "../core/interfaces.ts";

export class Genie<I> implements Faculty<I, string> {
    readonly quick: Sensitive<I> = new ReplaySubject<Sensation<I>>(1);
    protected sensations: Sensation<I>[] = [];

    constructor(
        protected name: string,
        protected description: string,
        protected instruction: string,
        protected narrate: (prompt: string) => Observable<string>,
    ) {
        logger.info(`Initializing Genie: ${name}`);
        this.initializeQuickSubscription();
        this.quick.subscribe((sensation) => {
            logger.info({ sensation }, `${name} received sensation`);
        });
        this.quick.next({
            when: new Date(),
            content: {
                explanation: `Initialized ${name}`,
                content: `Poke the quick to start the ${name}`,
            },
        });
    }

    protected initializeQuickSubscription() {
        logger.info(`${this.name}: Initializing quick subscription`);
        this.quick.pipe(
            tap((v) => logger.info({ v }, `${this.name} quick`)),
            throttleTime(500),
            toArray(),
            switchMap((sensations) => {
                logger.info(`${this.name}: Received sensations`);
                this.sensations = sensations;
                return this.consult();
            }),
        ).subscribe({
            next: (narration) =>
                logger.info({ narration }, `${this.name} narration`),
            error: (err) =>
                logger.error(err, `${this.name} encountered an error`),
        });
    }

    protected formatInput(
        input: Sensation<unknown>[],
    ): Record<string, unknown> {
        logger.info(`${this.name}: Formatting input`);
        return { sensations: input };
    }

    consult(): Observable<string> {
        logger.info(`${this.name}: Consulting`);
        const template = Handlebars.compile(
            `You are playing the role of {{name}}. {{description}} {{instruction}}`,
        );
        logger.info(`${this.name}: Compiling template`);
        const input = {
            name: this.name,
            description: this.description,
            instruction: this.instruction,
            sensations: this.formatInput(this.sensations),
        };

        // Adding introspective layer to enhance awareness
        const introspectionTemplate = Handlebars.compile(
            `Reflect on Pete's current state:
            {{#each sensations}}
            At {{when}}, Pete felt '{{explanation}}'.
            {{/each}}
            How do these sensations connect, and what do they tell us about Pete's emotional state?`,
        );
        const introspectivePrompt = introspectionTemplate(input);
        logger.info(
            { introspectivePrompt },
            `${this.name}: Introspective prompt generated`,
        );

        return this.narrate(introspectivePrompt).pipe(
            tap((response) =>
                logger.info(`${this.name}: LLM response received`, { response })
            ),
            catchError((err) => {
                logger.error(`${this.name}: Error invoking LLM`, err);
                return of("");
            }),
        );
    }
}
