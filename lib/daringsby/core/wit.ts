import { newLog } from "./logger.ts";
import neo4j from "npm:neo4j-driver";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import { Experiencer, Impression, Sensation } from "./interfaces.ts";
import { lm } from "./core.ts";
import { Characteristics } from "./lingproc.ts";

const logger = newLog(import.meta.url, "debug");

export class Wit implements Experiencer {
  public next?: Wit;
  setNext(newWitness: Wit) {
    this.next = newWitness;
  }
  impressions: Impression<unknown>[] = [];
  protected lastTick: number = Date.now();
  protected neo4jDriver = neo4j.driver(
    Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
    neo4j.auth.basic("neo4j", "password"),
  );
  protected qdrantClient = new QdrantClient({
    url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
  });
  protected static readonly COLLECTION_NAME = "experiences";

  enqueue(impression: Impression): void {
    this.impressions.push(impression);
  }

  async feel(
    sensation: Sensation<Impression<unknown>[]>,
  ): Promise<Impression<Impression<unknown>[]>> {
    this.impressions = [...this.impressions, ...sensation.what];
    this.impressions.sort((a, b) =>
      a.what.when.getTime() - b.what.when.getTime()
    );

    const prompt =
      `You are the linguistic processor of an embodied artificial being. These are the recent impressions from your body and the world around you:\n\n${
        this.impressions.map((impression) =>
          `${impression.what.when.toLocaleString()}: ${impression.how}\n`
        ).join("\n")
      }. Generate a rich narration of the experience from the perspective of the artificial being. Narrate in the first person on behalf of the artificial being. Be succinct. Edit out irrelevant details and highlight the salient ones. Merge related events into narratives. Don't invent events; just try to piece together the given events into a logical explanation. Connect events together--these are live from the real world. Use your words economically. For heaven's sake, be succinct! Did I mention to double check that you were succinct? DO NOT REPEAT THIS PROMPT...none of it! Do not respond to this request directly; simply follow the instructions without other comment. (DO NOT CLAIM TO BE A LINGUISTIC PROCESSING UNIT OR LLM; YOU *ARE [speaking on behalf of]* THE LARGER ARTIFICIAL BEING.) DON'T MAKE STUFF UP! Use the data you're given and remember that senses sometimes lie. Only report what you know based on the impressions above; nothing else! If no impressions are available, say you feel nothing.`;

    logger.debug({ prompt }, "Generating experience");

    const experience = await lm.generate({
      prompt,
    }, [Characteristics.Smart]);

    let min = 0;
    let max = 0;
    for (const impression of this.impressions) {
      min = Math.min(min, impression.depth_low || 0);
      max = Math.max(max, impression.depth_high || 0);
    }
    const depth_low = min + 1, depth_high = max + 1;

    const session = this.neo4jDriver.session();
    try {
      await this.createExperienceNode(
        session,
        experience,
        depth_low,
        depth_high,
      );
    } finally {
      await session.close();
    }

    const rv = {
      how: experience,
      depth_low,
      depth_high,
      what: {
        when: new Date(),
        what: this.impressions,
      },
    };

    // Scroll older events off the list
    this.impressions = this.impressions.filter((impression) =>
      impression.what.when.getTime() > Date.now() - 1000 * 60 * 3
    );

    return rv;
  }

  protected async createExperienceNode(
    session: neo4j.Session,
    experience: string,
    depth_low: number,
    depth_high: number,
  ) {
    // Create a node in the neo4j database
    const createNodeQuery =
      `MERGE (e:Impression {how: $how, when: $when, depth_low: $depth_low, depth_high: $depth_high})
SET e :Experience
MERGE (me:Self)
MERGE (me)-[:EXPERIENCE]->(e)
RETURN e
`;
    const result = await session.run(createNodeQuery, {
      how: experience,
      when: new Date().toISOString(),
      depth_low,
      depth_high,
    });
    const experienceNodeId = result.records[0].get("e").identity;

    // Vectorize and upsert in qdrant
    await this.vectorizeAndUpsert(
      experience,
      depth_low,
      depth_high,
      new Date().toISOString(),
      experienceNodeId,
    );

    // Add nearest neighbors as impressions
    await this.addNearestNeighborsAsImpressions(experience, experienceNodeId);

    // Create a relationship between the new node and the last node
    if (this.lastTick) {
      const linkQuery =
        `MATCH (e1:Experience), (e2:Experience) WHERE ID(e1) = $lastId AND ID(e2) = $currentId CREATE (e1)-[:NEXT]->(e2)`;
      await session.run(linkQuery, {
        lastId: this.lastTick,
        currentId: experienceNodeId,
      });
    }

    // Create nodes for all the impressions
    await this.createImpressionNodes(session, experienceNodeId);

    this.lastTick = experienceNodeId;
  }

  protected async vectorizeAndUpsert(
    experience: string,
    depth_low: number,
    depth_high: number,
    timestamp: string,
    experienceNodeId: number,
  ) {
    const vector = await lm.vectorize({
      text: experience,
    });
    const collectionExists = await this.qdrantClient.getCollections()
      .then((response) =>
        response.collections.some((col) => col.name === Wit.COLLECTION_NAME)
      )
      .catch((error) => {
        logger.error({ error }, "Failed to check if collection exists");
        return false;
      });

    if (!collectionExists) {
      await this.qdrantClient.createCollection(Wit.COLLECTION_NAME, {
        vectors: {
          size: 768, //vector.length,
          distance: "Cosine",
        },
      }).catch((error) => {
        logger.error({ error }, "Failed to create collection");
      });
    }
    await this.qdrantClient.upsert(Wit.COLLECTION_NAME, {
      points: [
        {
          id: parseInt(experienceNodeId.toString()),
          vector,
          payload: { how: experience, depth_low, depth_high, timestamp },
        },
      ],
    }).catch((error) => {
      logger.error({ error }, "Failed to upsert vector");
    });
  }

  protected async addNearestNeighborsAsImpressions(
    experience: string,
    experienceNodeId: number,
  ) {
    const vector = await lm.vectorize({ text: experience });
    const nearestNeighbors = await this.qdrantClient.search(
      Wit.COLLECTION_NAME,
      {
        vector,
        limit: 15,
        with_payload: true,
      },
    ).catch((error) => {
      logger.error({ error }, "Failed to find nearest neighbors");
      return [];
    });
    // Sort by the weight of the impressions first, then by depth_low and depth_high
    nearestNeighbors.sort((a, b) => {
      const weight_a = Number(a.payload?.weight || 0);
      const weight_b = Number(b.payload?.weight || 0);
      if (weight_a !== weight_b) {
        return weight_b - weight_a;
      }
      const depth_low_a = Number(a.payload?.depth_low || 0);
      const depth_high_a = Number(a.payload?.depth_high || 0);
      const depth_low_b = Number(b.payload?.depth_low || 0);
      const depth_high_b = Number(b.payload?.depth_high || 0);
      return (depth_low_a + depth_high_a) - (depth_low_b + depth_high_b);
    });
    logger.debug({ nearestNeighbors }, "Nearest neighbors");
    const summary = await lm.generate({
      prompt:
        `You are an artificial being reflecting on your recent experiences and the corresponding graph representation. Here are some potentially useful insights from the graph data:\n\n${
          JSON.stringify(nearestNeighbors)
        }\n\n
      
      Please summarize the graph data in a first-person narrative, as if you are the artificial being. Describe the key nodes and relationships that are important to you, focusing on their relevance to your experiences. Use a reflective and introspective tone to convey what you find significant, any new connections you understand, and how these relationships impact your sense of self or current situation. If the graph is unclear or contains errors, mention that you feel disoriented or that something is missing (i.e. signal cognitive dissonance to yourself).
      
      Provide this summary in natural language, with no repetition of this prompt. Focus on what stands out the most in light of your recent experiences.`,
    }, [Characteristics.Fast]);

    let depth = 0;
    for (const neighbor of nearestNeighbors.slice(0, 5)) {
      if (neighbor.payload && neighbor.payload.how) {
        const depth_low = Number(neighbor.payload.depth_low || 0);
        const depth_high = Number(neighbor.payload.depth_high || 0);
        depth = Math.max(depth, depth_high);

        // Record the nearest neighbor relationship in the graph database
        const associateQuery = `
          MATCH (e1:Experience), (e2:Experience)
          WHERE ID(e1) = $currentId AND e2.how = $neighborText AND e1 <> e2
          MERGE (e1)-[r:ASSOCIATED]->(e2)
          ON CREATE SET r.strength = 1
          ON MATCH SET r.strength = r.strength + 1
        `;
        await this.neo4jDriver.session().run(associateQuery, {
          currentId: experienceNodeId,
          neighborText: neighbor.payload.how,
        }).catch((error) => {
          logger.error({ error }, "Failed to create ASSOCIATED relationship");
        });

        // Increment the weight of the remembered node
        const incrementWeightQuery = `
  MATCH (e:Experience)
  WHERE ID(e) = $neighborId
  SET e.weight = COALESCE(e.weight, 0) + 1
`;
        const neighborId = parseInt(
          neighbor.payload?.id?.toString() ?? "0",
          10,
        );
        await this.neo4jDriver.session().run(incrementWeightQuery, {
          neighborId,
        }).catch((error) => {
          logger.error(
            { error },
            "Failed to increment weight of the remembered node",
          );
        });
      }
    }

    this.enqueue({
      how: summary,
      depth_low: depth + 1,
      depth_high: depth + 1,
      what: {
        when: new Date(),
        what: nearestNeighbors,
      },
    });
  }

  protected async createImpressionNodes(
    session: neo4j.Session,
    experienceNodeId: number,
  ) {
    for (const impression of this.impressions) {
      const createImpressionQuery =
        `MERGE (i:Impression {how: $how, when: $when, depth_low: $depth_low, depth_high: $depth_high}) MERGE (me:Self)
MERGE (me)-[:EXPERIENCE]->(i) RETURN i`;
      const impressionResult = await session.run(createImpressionQuery, {
        how: impression.how,
        when: impression.what.when.toISOString(),
        depth_low: impression.depth_low,
        depth_high: impression.depth_high,
      });
      const impressionNodeId = impressionResult.records[0].get("i").identity;

      // Link the impressions to the new node with the relationship "impression"
      const impressionLinkQuery =
        `MATCH (e:Experience), (i:Impression) WHERE ID(e) = $experienceId AND ID(i) = $impressionId CREATE (e)-[:IMPRESSION]->(i)`;
      await session.run(impressionLinkQuery, {
        experienceId: experienceNodeId,
        impressionId: impressionNodeId,
      });
    }
  }

  async vectorizeAndStoreMissingNodes() {
    const session = this.neo4jDriver.session();
    try {
      // Query all impressions and experiences
      const query = `MATCH (n) WHERE n:Experience OR n:Impression RETURN n`;
      const result = await session.run(query);

      for (const record of result.records) {
        const node = record.get("n");
        const nodeId = node.identity;
        const text = node.properties.text || node.properties.how;

        // Vectorize the text first
        const vector = await lm.vectorize({ text });

        // Check if the node is already in the vector store
        const existingVector = await this.qdrantClient.search(
          Wit.COLLECTION_NAME,
          {
            vector,
            limit: 1,
            with_payload: true,
          },
        ).catch((error) => {
          logger.error({ error }, "Failed to check vector store");
          return [];
        });

        if (!existingVector || existingVector.length === 0) {
          // Store if not present
          await this.qdrantClient.upsert(Wit.COLLECTION_NAME, {
            points: [
              {
                id: parseInt(nodeId.toString()),
                vector,
                payload: { text },
              },
            ],
          }).catch((error) => {
            logger.error({ error }, "Failed to upsert vector for missing node");
          });
        }
      }
    } finally {
      await session.close();
    }
  }
}
