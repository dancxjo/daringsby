export class FondDuCoeur extends Genie<string> {
    protected subscriptionStack: Subscription[] = [];
    protected wits: Wit[] = [];

    constructor() {
        super(
            "Fond du Coeur",
            `Fond du Coeur is Pete's emotional core. It integrates all the Wits, serves as the seat of his feelings, and provides a coherent and accurate account of the emotional data.`,
            `Narrate the emotional state and experience of Pete.
{{#sensations}}At {{when}}, {{explanation}}.
{{/sensations}}
`,
            defaultNarration,
        );
        logger.info("Fond du Coeur: Initializing wits");
        this.initializeWits();
    }

    private initializeWits() {
        this.wits = [
            new Wit("The Living Daylights", "Wit of the Instant", this),
            new Wit("Fourth Nonblond", "Wit of the Moment", this),
            new Wit("Contextualizer", "Broader Context Integration", this),
        ];
    }

    override consult(): Observable<string> {
        logger.info("Fond du Coeur: Consulting");
        return combineLatest(this.wits.map((wit) => wit.consult())).pipe(
            map((narrations) => narrations.join("\n")),
            tap((combinedNarration) => {
                logger.info("Fond du Coeur: Combined narration received");
                this.quick.next({
                    when: new Date(),
                    content: {
                        explanation: combinedNarration,
                        content: combinedNarration,
                    },
                });
            }),
            switchMap(() => super.consult()),
        );
    }

    unshift(intervalMs: number): void {
        logger.info(`Fond du Coeur: Unshifting with interval ${intervalMs}ms`);
        this.subscriptionStack.push(
            interval(intervalMs).pipe(
                switchMap(() => this.consult()),
                switchMap((narration) => {
                    this.quick.next({
                        when: new Date(),
                        content: {
                            explanation: narration,
                            content: narration,
                        },
                    });
                    return super.consult();
                }),
            ).subscribe({
                next: (narration) =>
                    logger.info({ narration }, "Fond du Coeur narration"),
                error: (err) =>
                    logger.error(err, "Fond du Coeur encountered an error"),
            }),
        );
    }
}
