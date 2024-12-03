import { newLog } from "./logger.ts";
import neo4j from "npm:neo4j-driver";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import {
  Experience,
  Experiencer,
  Impression,
  Sensation,
  Sensitive,
} from "./interfaces.ts";
import { lm } from "./core.ts";
import yml from "npm:yaml";

const logger = newLog(import.meta.url, "debug");

export class Contextualizer implements Sensitive<Experience[]> {
  protected neo4jDriver = neo4j.driver(
    Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
    neo4j.auth.basic("neo4j", "password"),
  );
  protected qdrantClient = new QdrantClient({
    url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
  });
  protected static readonly COLLECTION_NAME = "experiences";
  protected results: string = "";

  constructor(protected context: string = "MERGE (me:Self) RETURN me") {
    logger.debug("Contextualizer initialized");
    // Load the latest experiences from neo4j
    this.loadExperiences().then((experiences) => {
      logger.debug(`Loaded ${experiences.length} experiences`);
      const sensation: Sensation<Experience[]> = {
        when: new Date(),
        what: experiences,
      };
      this.feel(sensation).then((impression) => {
        logger.debug("Contextualizer initialized");
      });
    });
  }

  async loadExperiences(): Promise<Experience[]> {
    const session = this.neo4jDriver.session();
    try {
      const query =
        `MATCH (e:Experience) RETURN e ORDER BY e.when DESC LIMIT 10`;
      const result = await session.run(query);
      return result.records.map((record) => record.get("e").properties);
    } catch (e) {
      return [{
        how:
          `I feel...foggy somehow. Like, I'm having trouble remembering what I was doing. Where am I? What's going on? ${
            (e as any).message ? (e as any).message : e
          }`,
        depth_low: 0,
        depth_high: 0,
        what: {
          when: new Date(),
          what: [],
        },
      }];
    } finally {
      session.close();
    }
  }

  async feel(
    sensation: Sensation<Experience[]>,
  ): Promise<Impression<Experience[]>> {
    // Feed the experiences to the language model
    const experiences = yml.stringify(sensation);
    const prompt =
      `Here are the recent experiences of an artificial being: ${experiences}\n\nConsider all the entities and relationships in these experiences. We need to represent these experiences in a graph database. Please write a Cypher query that will merge these experiences into the graph. You can assume that the experiences are already in the database. The query should return the node representing the artificial being and all entities relevant to the situation represented in the experiences. Be wary of creating new nodes or relationships unnecessarily; use MERGE insted of CREATE. Remember, the goal is to represent the experiences in the graph, to create new data in the graph and to update old data. A query that is always appropriate is: MERGE (me:Self) RETURN me. Include that at the bare minimum. Example: Experience: "I see a pretty yellow cat." Cypher: "MERGE (me:Self) MERGE (cat:Cat {color: 'yellow'}) RETURN me, cat\n\nIMPORTANT: Your response will be executed immediately. It must be valid and correct cypher. Do not include comments explanations or dithering. Make the query work or crash yourself.\nThe context was previously: ${this.context}\n\nThe results of that query are: ${this.results}\n\nNew query or repeated last query:\n\n`;

    const response = (await lm.generate({ prompt })).replace(/```\s*$/g, "");
    logger.debug({ response }, `Response`);

    const depth_low = sensation.what.reduce((acc, exp) => {
      return acc + (exp.depth_low ?? 0);
    }, 0);
    const depth_high = sensation.what.reduce((acc, exp) => {
      return acc + (exp.depth_high ?? 0);
    }, 0);

    this.attemptNewContext(response);

    return {
      how: response,
      depth_low,
      depth_high,
      what: sensation,
    };
  }

  async attemptNewContext(response: string): Promise<void> {
    logger.debug({ response }, "Attempting new context");
    const session = this.neo4jDriver.session();
    try {
      const result = await session.run(response);
      this.results = JSON.stringify(
        result.records.map((record) => record.toObject()),
      );
      logger.debug({ result: this.results }, "Results");
    } catch (e) {
      this.results = `Error: ${(e as any).message ? (e as any).message : e}`;
      logger.error({ error: this.results }, "Error");
    } finally {
      session.close();
    }
  }

  async getContext(): Promise<string> {
    const prompt =
      `These are the recent experiences of an artificial being and possibly relevant memory records. The following query resulted in the following graph: ${this.context} ${this.results}\n\nNarrate the nodes/relationships from the graph to the artificial being in natural language. Use the first person, as if you are the artificial being speaking to itself. Disregard irrelevant nodes and relationships. Highlight what is important in light of the experiences. Do not repeat this prompt: simply provide the summary of the graph in natural language. If the results are in error, say that no bells are ringing.`;
    const response = await lm.generate({ prompt });
    logger.debug({ response }, "Response to getContext");
    return response;
  }
}
