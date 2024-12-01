import { logger } from "../lib/daringsby/core/logger.ts";
import neo4j from "npm:neo4j-driver";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import { Config, EmbeddingsResponse, Ollama } from "npm:ollama";
import {
  Experiencer,
  Impression,
  Sensation,
} from "../lib/daringsby/core/interfaces.ts";
import { lm } from "../lib/daringsby/core/core.ts";

interface Document<T> {
  metadata: {
    label: string;
  };
  data: T;
}

export class Witness implements Experiencer {
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
    sensation: Sensation<Impression[]>,
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
      }. Generate a rich narration of the experience from the perspective of the artificial being. Narrate in the first person on behalf of the artificial being. Be succinct. Edit out irrelevant details and highlight the salient ones. Merge related events into narratives. Let's imagine you were to feel the keys spell something out. Don't invent events; just try to piece together the given events into a logical explanation. Connect events together. If you see someone, they might be the same someone you feel pressing your keys; they might be trying to communicate with you. Use your words economically.`;

    logger.info({ prompt }, "Generating experience");

    const experience = await lm.generate({
      prompt,
    });

    const session = this.neo4jDriver.session();
    try {
      // Create a node in the neo4j database
      const createNodeQuery =
        `CREATE (e:Experience {text: $text, timestamp: $timestamp}) RETURN e`;
      const result = await session.run(createNodeQuery, {
        text: experience,
        timestamp: new Date().toISOString(),
      });
      const experienceNodeId = result.records[0].get("e").identity;

      // Vectorize a copy of that node
      const vector = await lm.vectorize({
        text: experience,
      });
      await this.qdrantClient.upsert(Witness.COLLECTION_NAME, {
        points: [
          {
            id: experienceNodeId.toString(),
            vector,
            payload: { text: experience },
          },
        ],
      }).catch((error) => {
        logger.error({ error }, "Failed to upsert vector");
      });

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

      this.lastTick = experienceNodeId;
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
}
