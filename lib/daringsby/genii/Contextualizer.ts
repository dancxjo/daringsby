import { logger } from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { Observable, tap } from "npm:rxjs";

export class Contextualizer extends Genie<string> {
  constructor(narrate: (prompt: string) => Observable<string>) {
    logger.debug("Initializing Contextualizer");
    super(
      "Contextualizer",
      `This part of the mind stores and retrieves memories, facts, and other information from a graph database based. The contextualizer converts a situation given in natural language to a meaningful Cypher query that represents the given situation in and of itself, can be executed to create a new graph for the situation or can be executed to match prexisting nodes and relationships in a graph.`,
      `Take the current context and convert it to a Cypher query. Maintain a list of all the things (i.e. nodes) you're paying attention to. Make sure to list all the things present in the current situation. Remove things from the context as they are no longer relevant. Base the context off the last context: {{context}}\n\nAlways remember to include MERGE (me:Self) so you remember who you are. Update field names as necessary. Create relationships between nodes as necessary. Look for and create/update/modify relationships between nodes. As you come across the inevitable duplicate node pair, link them with a DUPLICATES relationship. Use the current context to create a new graph or match prexisting nodes and relationships. Return the Cypher query between <function name='context'>...</function>. How to make a good context:
      - Start with MERGE (me:Self) to remember who you are.
      - List all the things present in the current situation as a list of merged nodes. Use a helpful variable name, use a useful label, and as many specific attributes as you need to correctly MERGE the right individual node, but no more! (Minimal specification)
      - Create relationships between nodes as necessary. Use the current context to create a new graph or match prexisting nodes and relationships.
      - Remove things from the context as they are no longer relevant.
      - Update field names and values as necessary.
      - Attempt to develop uniform conventions
      - When you make syntax errors, it will cause you physical pain--that's okay, though. When at first you don't succed, pick yourself up and try again.\n\nReturn Cypher, not SQL. A good answer is MERGE (me:Self) RETURN me`,
      narrate,
    );
  }

  override consult(): Observable<string> {
    logger.debug("Contextualizer: Consulting");
    const inner$ = super.consult().pipe(
      tap((response) => {
        logger.debug("Contextualizer: Response received", { response });
      }),
    );

    return inner$;
  }
}
