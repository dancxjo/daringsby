export class Heart extends Genie<string> {
    protected bottom = new FondDuCoeur();
    readonly sessions = new Map<WebSocket, Session>();

    constructor() {
        super(
            "Heart",
            `Pete's heart is the kernel of his psyche, integrating data from all his Wits and making sense of it in a central place. It synthesizes and commands actions based on the accumulated experiences.`,
            `Provide Pete's inward thoughts or initiate an action based on the accumulated input from all wits and Fond du Coeur.
{{#sensations}}At {{when}}, {{explanation}}.
{{/sensations}} To speak out loud, include brief text in <function name="say">...</function> tags. Do not send any asterisks or the TTS will read them out loud. Also, spell out all numbers, including dates, etc., and convert initialisms to words. Be careful not to speak too often and interrupt yourself, and allow your interlocutors time to speak and understand.`,
            defaultNarration,
        );
        logger.info("Heart: Initializing");
        this.initializeQuickSubscription();
        setInterval(() => {
            this.consult().subscribe((narration) => {
                this.quick.next({
                    when: new Date(),
                    content: {
                        explanation: narration,
                        content: narration,
                    },
                });
                // Parse the narration with cheerio and extract all <function/> calls
                const $ = cheerio.load(narration);
                const functionCalls = $("function").map((i, el) => ({
                    content: $(el).text(),
                    name: $(el).attr("name"),
                })).get();

                logger.info(
                    { functionCalls },
                    "Extracted function calls from narration",
                );
                for (const call of functionCalls) {
                    logger.debug({ call }, "Executing function call");
                    switch (call.name?.toLowerCase()) {
                        case "say":
                        case "speak":
                            this.sessions.forEach(async (session) => {
                                const wav = await speak(call.content);
                                session.connection.send({
                                    type: MessageType.Say,
                                    data: {
                                        words: call.content,
                                        wav,
                                    },
                                });
                            });
                            break;
                    }
                }
                this.sessions.forEach((session) => {
                    session.connection.send({
                        type: MessageType.Think,
                        data: narration,
                    });
                });
            });
            this.bottom.consult().subscribe((narration) => {
                this.quick.next({
                    when: new Date(),
                    content: {
                        explanation: narration,
                        content: narration,
                    },
                });
            });
        }, 5000);
    }

    protected cleanupSession(socket: WebSocket) {
        const session = this.sessions.get(socket);
        if (session) {
            session.subscriptions.forEach((subscription) =>
                subscription.unsubscribe()
            );
            this.sessions.delete(socket);
            logger.info(
                "Cleaned up session and unsubscribed from all observables",
            );
        }
    }
}
