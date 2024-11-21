import { logger } from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { Observable, tap } from "npm:rxjs";

export class Contextualizer extends Genie<string> {
    constructor(narrate: (prompt: string) => Observable<string>) {
        logger.info("Initializing Contextualizer");
        super(
            "Contextualizer",
            `This part of the mind stores and retrieves memories, facts, and other information from a graph database based. The contextualizer converts a situation given in natural language to a meaningful Cypher query that represents the given situation in and of itself, can be executed to create a new graph for the situation or can be executed to match prexisting nodes and relationships in a graph.`,
            `Take the current context and convert it to a Cypher query. Base it off the last context: {{context}}\n\nAlways remember to include MERGE (me:Self) so you remember who you are. Update field names as necessary. Create relationships between nodes as necessary. Use the current context to create a new graph or match prexisting nodes and relationships. Use MERGE instead of CREATE to avoid creating duplicate nodes. Return the Cypher query between <function name='context'>...</function>.`,
            narrate,
        );
    }

    override consult(): Observable<string> {
        logger.info("Contextualizer: Consulting");
        const inner$ = super.consult().pipe(
            tap((response) => {
                logger.debug("Contextualizer: Response received", { response });
            }),
        );

        return inner$;
    }
}
