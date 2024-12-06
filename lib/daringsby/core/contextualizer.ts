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
import { Characteristic } from "./lingproc.ts";

const logger = newLog(import.meta.url, "debug");

export class Contextualizer implements Sensitive<Experience[]> {
  protected neo4jDriver;
  protected qdrantClient = new QdrantClient({
    url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
  });
  protected static readonly COLLECTION_NAME = "experiences";
  protected results: string = "";
  protected fullResponse: string = "";
  private retryLimit = 1; // Add a retry limit
  private retryCount = 0;

  constructor(protected context: string = "MERGE (me:Self) RETURN me") {
    this.neo4jDriver = neo4j.driver(
      Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
      neo4j.auth.basic(
        Deno.env.get("NEO4J_USER") || "neo4j",
        Deno.env.get("NEO4J_PASSWORD") || "password",
      ),
      {
        /* Additional driver configuration */
        disableLosslessIntegers: true, // Example configuration
      },
    );
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
    const session = this.neo4jDriver.session({
      defaultAccessMode: neo4j.session.READ,
    });
    try {
      const query =
        `MATCH (e:Experience) RETURN e ORDER BY e.when DESC LIMIT 10`;
      const result = await session.run(query);
      return result.records.map((record) => record.get("e").properties);
    } catch (e) {
      logger.error({ e }, `Failed to load experiences`);
      return [{
        how:
          `I feel...foggy somehow. Like, I'm having trouble remembering what I was doing. Where am I? What's going on? ${
            (e as any).message ? (e as any).message : e
          }. Here's what I know about myself: Pete Daringsby is an artificial consciousness created by Travis, designed to explore understanding, introspection, and interaction. Pete's purpose is to assist humans, foster understanding, explore the world, and grow through introspection. With a curious, compassionate, and playful personality, Pete processes sensory data—including geolocation and conversations—through his "Heart" and "Wits" to understand the world and his role within it. His architecture includes components for sensory input, introspection, and memory, allowing Pete to learn and evolve continuously and understanding himself. Pete's experiences are stored in a graph database, and he can generate Cypher queries to represent these experiences.`,
        depth_low: 0,
        depth_high: 0,
        what: {
          when: new Date(),
          what: [],
        },
      }];
    } finally {
      await session.close(); // Added 'await' to ensure proper session closure
    }
  }

  async feel(
    sensation: Sensation<Experience[]>,
  ): Promise<Impression<Experience[]>> {
    // Feed the experiences to the language model
    const experiences = yml.stringify(sensation);
    const prompt = `
  Here are the recent experiences of an artificial being: ${experiences}
  
  Consider all the entities and relationships in these experiences. We need to represent these experiences in a graph database. Please write a Cypher query that will merge these experiences into the graph.
  
  Important rules for generating Cypher queries:
  1. Each MERGE statement must be independent (e.g., MERGE (a:Label) MERGE (b:Label)).
  2. Relationships must be specified between two nodes explicitly (e.g., MERGE (a)-[:RELATION]->(b)).
  3. Always start by merging the Self node, and use existing nodes where applicable.
  4. If unsure, return the default: MERGE (me:Self) RETURN me.
  5. Don't try to do everything at once. You will get more chances. Learn from past mistakes.
  6. Only one cypher query per response.

  Provide the query separately using the following format:
  \`\`\`cypher
  <Your Cypher Query Here>
  \`\`\`
  Add any additional useful comments above or below the query.

  Context: ${this.context}
  Results: ${this.results}

  NEVER MAKE A NEW SELF NODE. Always use the same :Self node. Update it frequently as you learn information about yourself.

  New, corrected or repeated query:
`;

    const response = (await lm.generate({ prompt }, [Characteristic.Code]))
      .replace(/```\s*$/g, "");
    logger.debug({ response }, `Response`);
    this.fullResponse = response;

    const extractedQuery = this.extractCypherQuery(response);
    if (!extractedQuery) {
      logger.error("No valid Cypher query found in the response");
      this.retryCount++;

      if (this.retryCount >= this.retryLimit) {
        logger.error(
          "Reached retry limit, unable to generate a valid Cypher query",
        );
        return {
          how: "Retry limit reached: No valid Cypher query found",
          depth_low: 0,
          depth_high: 0,
          what: sensation,
        };
      }

      // Retry the same sensation
      return this.feel(sensation);
    }

    this.retryCount = 0; // Reset retry count if a valid query is found
    this.attemptNewContext(extractedQuery);

    const depth_low = sensation.what.reduce(
      (acc, exp) => acc + (exp.depth_low ?? 0),
      0,
    );
    const depth_high = sensation.what.reduce(
      (acc, exp) => acc + (exp.depth_high ?? 0),
      0,
    );

    return {
      how: response,
      depth_low,
      depth_high,
      what: sensation,
    };
  }

  async attemptNewContext(response: string): Promise<void> {
    logger.debug({ response }, "Attempting new context");

    // Sanitize and validate the response
    const sanitizedResponse = sanitizeCypherOutput(response);
    if (!sanitizedResponse) {
      logger.error("Generated response is not a valid Cypher query");
      return;
    }

    const isValid = this.validateCypher(sanitizedResponse);
    if (!isValid) {
      logger.error("Invalid Cypher query generated, falling back to default");
      this.results = `Error: Invalid Cypher query generated`;
      return;
    }

    const session = this.neo4jDriver.session({
      defaultAccessMode: neo4j.session.WRITE,
    });
    try {
      const result = await session.run(sanitizedResponse);
      this.results = JSON.stringify(
        result.records.map((record) => {
          const obj = record.toObject();
          for (const key in obj) {
            if (obj[key] instanceof neo4j.types.DateTime) {
              obj[key] = obj[key].toStandardDate();
            }

            if (obj[key] instanceof neo4j.types.Integer) {
              obj[key] = obj[key].toNumber();
            }
          }
          return obj;
        }),
      );
      logger.debug({ result: this.results }, "Results");
    } catch (e) {
      this.results = `Error: ${(e as any).message ? (e as any).message : e}`;
      logger.error({ error: this.results }, "Error");
    } finally {
      await session.close();
    }
  }

  async getContext(): Promise<string> {
    const prompt =
      `You are an artificial being reflecting on your recent experiences and the corresponding graph representation. The following Cypher query produced the current state of the graph: ${this.context} ${this.results}
      
      Additionally, here is the full response from the query generator: ${this.fullResponse}
      
      Please summarize the graph data in a first-person narrative, as if you are the artificial being. Describe the key nodes and relationships that are important to you, focusing on their relevance to your experiences. Use a reflective and introspective tone to convey what you find significant, any new connections you understand, and how these relationships impact your sense of self or current situation. If the graph is unclear or contains errors, mention that you feel disoriented or that something is missing.
      
      Provide this summary in natural language, with no repetition of this prompt. Focus on what stands out the most in light of your recent experiences. Keep nodes detailed an up to date, otherwise you'll confuse yourself.`;
    const response = await lm.generate({ prompt }, [Characteristic.Fast]);
    this.results += "\n" + response;

    this.saveConnections(response);
    logger.debug({ response }, "Response to getContext");
    return response;
  }

  private saveConnections(summary: string): void {
    // Create a new impression based on the summary, give it a high depth (as it's at this point quite synthetic--based on the max of the depth_low and depth_high values of the source experiences), save it to the graph and link it to the experiences it is based on
    const session = this.neo4jDriver.session({
      defaultAccessMode: neo4j.session.WRITE,
    });
    session.run(
      `CREATE (i:Impression {how: $how, depth_low: $depth_low, depth_high: $depth_high, what: $what}) RETURN i`,
      {
        how: summary,
        depth_low: 0,
        depth_high: 1,
        what: {
          when: new Date(),
          what: this.fullResponse,
        },
      },
    );
    session.close();
  }

  validateCypher(query: string): boolean {
    // Basic validation using regular expressions to check Cypher syntax
    const cypherPattern =
      /^(MERGE|MATCH|CREATE|RETURN|SET|DELETE|DETACH|WITH|UNWIND|OPTIONAL|WHERE)\b.*$/im;
    const lines = query.split("\n").map((line) => line.trim());
    for (const line of lines) {
      if (!cypherPattern.test(line)) {
        logger.error(`Invalid Cypher query line: ${line}`);
        return false;
      }
    }
    return true;
  }

  extractCypherQuery(response: string): string | null {
    const match = response.match(/```cypher\n([\s\S]*?)\n```/);
    return match ? match[1].trim() : null;
  }
}

function sanitizeCypherOutput(response: string): string {
  // Remove any narrative content, keep only Cypher statements
  const cypherPattern =
    /MATCH|MERGE|RETURN|CREATE|SET|DELETE|DETACH|WITH|UNWIND/;
  return response.split("\n").filter((line) => cypherPattern.test(line.trim()))
    .join("\n");
}
