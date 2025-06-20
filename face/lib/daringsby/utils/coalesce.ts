import { logger } from "../core/logger.ts";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import { Ollama } from "npm:ollama";
import { vectorize } from "./memory.ts";
import { v4 } from "npm:uuid";

const qdrant = new QdrantClient({
  url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
});
const COLLECTION_NAME = "moments";

/**
 * Coalesces nearby vector points in the Qdrant collection.
 * Points within a distance threshold are merged into a single representative point.
 * Summarizes merged points using an external LLM (e.g., Llama3.2).
 */
export async function coalesceVectors(d = 0.1, n = 10) {
  try {
    logger.info("Starting vector coalescence");

    const ollama = new Ollama({
      host: Deno.env.get("OLLAMA_URL") || "http://localhost:11434",
    });

    // Get total number of points in the collection
    const totalCountResponse = await qdrant.count(COLLECTION_NAME);
    const totalPoints = totalCountResponse.count;
    logger.info(
      { totalPoints },
      "Total points in collection before coalescence",
    );

    while (true) {
      // Fetch any point from the collection
      const pointsResponse = await qdrant.scroll(COLLECTION_NAME, {
        limit: 1,
        with_vector: true,
        with_payload: true,
      });

      const points = pointsResponse.points;

      if (!points || points.length === 0) {
        logger.info("No more points to process. Coalescence complete.");
        break;
      }

      const point = points[0];

      // Find nearest neighbors
      const neighborsResponse = await qdrant.search(COLLECTION_NAME, {
        vector: point.vector as number[],
        limit: n,
        with_payload: true,
      });

      const neighbors = neighborsResponse.filter(
        (neighbor) => neighbor.id !== point.id && neighbor.score <= d,
      );

      if (neighbors.length === 0) {
        // logger.info(
        //   { count: points.length },
        //   "No neighbors within threshold. Moving to next point.",
        // );
        continue;
      }

      // Summarize merged payloads using LLM
      const payloads = [
        point.payload,
        ...neighbors.map((neighbor) => neighbor.payload),
      ];
      const summaryResponse = await ollama.generate({
        prompt:
          `Summarize the following thoughts into a cohesive statement. Use the first person (according to the orginals). Only say the summary without a preface.\n${
            JSON.stringify(payloads)
          }`,
        model: "llama3.2",
      });

      const summary = summaryResponse.response;
      logger.info({ summary }, "Generated summary for merged points");

      // Coalesce points
      const mergedVector = (await vectorize(summary)).embedding;

      const mergedPayload = {
        summary,
        mergedFrom: neighbors.map((neighbor) => neighbor.id),
      };

      // Create new representative point
      const mergedId = v4();
      await qdrant.upsert(COLLECTION_NAME, {
        points: [
          {
            id: mergedId,
            vector: mergedVector as number[],
            payload: mergedPayload,
          },
        ],
      });

      // Delete old points
      const idsToDelete = [
        point.id,
        ...neighbors.map((neighbor) => neighbor.id),
      ];
      await qdrant.delete(COLLECTION_NAME, { points: idsToDelete });

      logger.info(
        { mergedId, idsToDelete },
        "Coalesced points into a new representative vector",
      );

      // Log updated total count
      const updatedCountResponse = await qdrant.count(COLLECTION_NAME);
      const updatedTotalPoints = updatedCountResponse.count;
      logger.info({ updatedTotalPoints }, "Updated total points in collection");
    }
  } catch (error) {
    logger.error({ error }, "Error during vector coalescence:");
    throw error;
  }
}

// Example usage (await coalesceVectors())
export async function startCoalescence() {
  await coalesceVectors(0.9, 5).catch((error) => {
    logger.error({ error }, "Error during coalescence:");
  });
}
