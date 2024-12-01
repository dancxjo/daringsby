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

    const session = this.neo4jDriver.session();
    try {
      await this.createExperienceNode(session, experience);
    } finally {
      await session.close();
    }

    const rv = {
      how: experience,
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
  ) {
    // Create a node in the neo4j database
    const createNodeQuery =
      `CREATE (e:Experience {text: $text, timestamp: $timestamp}) RETURN e`;
    const result = await session.run(createNodeQuery, {
      text: experience,
      timestamp: new Date().toISOString(),
    });
    const experienceNodeId = result.records[0].get("e").identity;

    // Vectorize and upsert in qdrant
    await this.vectorizeAndUpsert(experience, experienceNodeId);

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
          payload: { text: experience },
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
        limit: 3,
        with_payload: true,
      },
    ).catch((error) => {
      logger.error({ error }, "Failed to find nearest neighbors");
      return [];
    });
    logger.info({ nearestNeighbors }, "Nearest neighbors");
    for (const neighbor of nearestNeighbors) {
      if (neighbor.payload && neighbor.payload.text) {
        this.enqueue({
          how:
            `I am reminded of a memory from ${neighbor.payload.timestamp}: ${neighbor.payload.text}`,
          what: {
            when: new Date(),
            what: neighbor,
          },
        });

        // Record the nearest neighbor relationship in the graph database
        const associateQuery = `
          MATCH (e1:Experience), (e2:Experience)
          WHERE ID(e1) = $currentId AND e2.text = $neighborText AND e1 <> e2
          MERGE (e1)-[r:ASSOCIATED]->(e2)
          ON CREATE SET r.strength = 1
          ON MATCH SET r.strength = r.strength + 1
        `;
        await this.neo4jDriver.session().run(associateQuery, {
          currentId: experienceNodeId,
          neighborText: neighbor.payload.text,
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
        `CREATE (i:Impression {how: $how, when: $when}) RETURN i`;
      const impressionResult = await session.run(createImpressionQuery, {
        how: impression.how,
        when: impression.what.when.toISOString(),
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
