import { newLog } from "../lib/daringsby/core/logger.ts";
import neo4j from "npm:neo4j-driver";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import {
  Experiencer,
  Impression,
  Sensation,
} from "../lib/daringsby/core/interfaces.ts";
import { lm } from "../lib/daringsby/core/core.ts";

const logger = newLog(import.meta.url, "info");

export class Witness implements Experiencer {
  protected next?: Witness;
  setNext(newWitness: Witness) {
    this.next = newWitness;
  }
  protected impressions: Impression<unknown>[] = [];
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
      `You are the linguistic processor of an embodied artificial being. These are the impressions of the sensations you have recently felt:\n\n${
        this.impressions.map((impression) =>
          `${impression.what.when.toLocaleString()}: ${impression.how}\n`
        ).join("\n")
      }. Generate a rich narration of the experience from the perspective of the artificial being. Narrate in the first person on behalf of the artificial being. Be succinct. Edit out irrelevant details and highlight the salient ones. Merge related events into narratives. Let's imagine you were to feel the keys spell something out. Don't invent events; just try to piece together the given events into a logical explanation. Connect events together. If you see someone, they might be the same someone you feel pressing your keys; they might be trying to communicate with you. Use your words economically. For heaven's sake, be succinct! Did I mention to double check that you were succinct?`;

    logger.debug({ prompt }, "Generating experience");

    const experience = await lm.generate({
      prompt,
    });

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
      `MERGE (e:Impression {how: $how, when: $when, depth_low: $depth_low, depth_high: $depth_high}) SET e :Experience RETURN e`;
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
        response.collections.some((col) => col.name === Witness.COLLECTION_NAME)
      )
      .catch((error) => {
        logger.error({ error }, "Failed to check if collection exists");
        return false;
      });

    if (!collectionExists) {
      await this.qdrantClient.createCollection(Witness.COLLECTION_NAME, {
        vectors: {
          size: vector.length,
          distance: "Cosine",
        },
      }).catch((error) => {
        logger.error({ error }, "Failed to create collection");
      });
    }
    await this.qdrantClient.upsert(Witness.COLLECTION_NAME, {
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
      Witness.COLLECTION_NAME,
      {
        vector,
        limit: 15,
        with_payload: true,
      },
    ).catch((error) => {
      logger.error({ error }, "Failed to find nearest neighbors");
      return [];
    });
    // Sort by the depth_low and depth_high of the impressions; we want to prefer more synthetic responses
    nearestNeighbors.sort((a, b) => {
      const depth_low_a = Number(a.payload?.depth_low || 0);
      const depth_high_a = Number(a.payload?.depth_high || 0);
      const depth_low_b = Number(b.payload?.depth_low || 0);
      const depth_high_b = Number(b.payload?.depth_high || 0);
      return depth_low_a + depth_high_a - depth_low_b - depth_high_b;
    });
    // logger.info({ nearestNeighbors }, "Nearest neighbors");
    for (const neighbor of nearestNeighbors.slice(0, 2)) {
      if (neighbor.payload && neighbor.payload.how) {
        const depth_low = Number(neighbor.payload.depth_low || 0);
        const depth_high = Number(neighbor.payload.depth_high || 0);
        this.enqueue({
          how:
            `I am reminded of a memory from ${neighbor.payload.when}: ${neighbor.payload.how}`,
          depth_low: depth_low + 1,
          depth_high: depth_high + 1,
          what: {
            when: new Date(),
            what: neighbor,
          },
        });

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
      }
    }
  }

  protected async createImpressionNodes(
    session: neo4j.Session,
    experienceNodeId: number,
  ) {
    for (const impression of this.impressions) {
      const createImpressionQuery =
        `MERGE (i:Impression {how: $how, when: $when, depth_low: $depth_low, depth_high: $depth_high}) RETURN i`;
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
          Witness.COLLECTION_NAME,
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
          await this.qdrantClient.upsert(Witness.COLLECTION_NAME, {
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

// Call the new method to vectorize and store missing nodes
const witness = new Witness();
witness.vectorizeAndStoreMissingNodes().catch((error) => {
  logger.error({ error }, "Failed to vectorize and store missing nodes");
});
