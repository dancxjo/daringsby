export class Wit extends Genie<string> {
    constructor(
        name: string,
        description: string,
        protected fondDuCoeur: FondDuCoeur,
    ) {
        super(
            name,
            `${description}: Integrates input from Fond du Coeur and provides a refined perspective on Pete's experiences.`,
            `Narrate the sensory data and refine its meaning in the context of ${name}.
{{#sensations}}At {{when}}, {{explanation}}.
{{/sensations}}
`,
            defaultNarration,
        );
        logger.info(`Wit: ${name} initialized`);
        this.initializeQuickSubscription();
    }

    override consult(): Observable<string> {
        logger.info(`Wit: ${this.name} consulting`);
        return this.quick.pipe(
            toArray(),
            debounceTime(100),
            switchMap((sensations) => {
                logger.info(`Wit: ${this.name} received sensations`);
                this.sensations = sensations;
                return super.consult();
            }),
        );
    }
}
