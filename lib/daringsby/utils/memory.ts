import { logger } from "../core/logger.ts";
import neo4j from "npm:neo4j-driver";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import { Config, EmbeddingsResponse, Ollama } from "npm:ollama";
import { v4 } from "npm:uuid";

interface Document<T> {
  metadata: {
    label: string;
  };
  data: T;
}

// Neo4j Configuration
const driver = neo4j.driver(
  Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
  neo4j.auth.basic("neo4j", "password"),
);

function createSession() {
  return driver.session({ defaultAccessMode: neo4j.session.WRITE });
}

// Qdrant Configuration
const qdrant = new QdrantClient({
  url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
});
const COLLECTION_NAME = "embeddings";

// Initialize Qdrant Collection
async function initializeQdrantCollection(): Promise<void> {
  try {
    logger.info("Initializing Qdrant collection");
    await qdrant.createCollection(COLLECTION_NAME, {
      vectors: {
        size: 768, // Adjust based on your embedding model
        distance: "Cosine",
      },
    });
    logger.info("Qdrant collection initialized successfully");
  } catch (error) {
    logger.error("Error initializing Qdrant collection:", error);
    throw error;
  }
}

/**
 * Stores a document in both Neo4j (document data only) and Qdrant (embedding).
 */
export async function memorize<
  T extends Record<string, string | number | boolean | unknown[]> = Record<
    string,
    string | number | boolean | unknown[]
  >,
>(
  document: Document<T>,
): Promise<void> {
  if (!document || !document.data) {
    logger.warn("No document data provided for memorization");
    return;
  }
  try {
    // Generate embedding using Ollama
    const url = Deno.env.get("OLLAMA_URL") || "http://localhost:11434";
    const ollama = new Ollama({
      host: url,
    });
    const embedding = await ollama.embeddings({
      prompt: JSON.stringify(document.data),
      model: "nomic-embed-text",
    });
    logger.debug({ embedding }, "Embedding generated successfully");

    // Store document in Neo4j and get the generated node ID
    // const nodeId = await storeDocumentInNeo4j(document);
    const nodeId = v4();
    // Store embedding in Qdrant using the node ID
    await qdrant.upsert(COLLECTION_NAME, {
      points: [
        {
          id: nodeId, // Convert Neo4j's integer ID to string for Qdrant
          vector: embedding.embedding,
          payload: { ...document },
        },
      ],
    });
    logger.info("Document stored in Neo4j and embedding stored in Qdrant");
  } catch (error) {
    logger.error({ error }, "Error storing document and embedding:", error);
  }
}

/**
 * Returns the results of a cypher query.
 */
export async function executeCypherQuery(query: string) {
  const sessionInstance = createSession();
  const tx = sessionInstance.beginTransaction();
  try {
    await tx.run(query);
    const result = await tx.run(query);
    await tx.commit();
    return result.records;
  } catch (error) {
    await tx.rollback();
    logger.error({ error }, "Error executing Cypher query:");
    throw error;
  } finally {
    await sessionInstance.close();
  }
}

/**
 * Stores the document in Neo4j without embedding data and returns the node ID.
 */
async function storeDocumentInNeo4j<T>(
  document: Document<T>,
): Promise<number> {
  const sessionInstance = createSession();
  const tx = sessionInstance.beginTransaction();
  try {
    const flattenedData = document.data as Record<string, unknown>;
    const dataEntries = Object.entries(flattenedData)
      .map(([key, value]) => `\`${key}\`: $${key}`)
      .join(", ");

    const query = `
      CREATE (doc:${document.metadata.label} { ${dataEntries} })
      RETURN id(doc) AS nodeId
    `;

    const result = await tx.run(query, flattenedData);

    await tx.commit();

    if (result.records.length === 0) {
      throw new Error("Document node creation failed");
    }

    const nodeId = result.records[0].get("nodeId").toNumber();
    logger.debug({ nodeId }, "Document stored in Neo4j successfully");
    return nodeId;
  } catch (error) {
    await tx.rollback();
    logger.error({ error }, "Error storing document in Neo4j:");
    throw error;
  } finally {
    await sessionInstance.close();
  }
}

const recentlyRecalled = new Map<string, Date>();

/**
 * Recalls the top k nodes from Qdrant based on a given prompt.
 */
export async function recall(prompt: string, k: number = 10): Promise<any[]> {
  logger.info({ prompt }, `Recalling information for prompt: ${prompt}`);
  if (!prompt) {
    logger.warn("No prompt provided for recall");
    return [];
  }
  logger.info({ prompt }, "Recalling information for prompt:");
  try {
    const ollama = new Ollama({
      host: Deno.env.get("OLLAMA_URL") || "http://localhost:11434",
    });
    const promptEmbedding: EmbeddingsResponse = await ollama.embeddings({
      prompt,
      model: "nomic-embed-text",
    });

    logger.info({ promptEmbedding }, "Embedding generated successfully");
    const response = await qdrant.search(COLLECTION_NAME, {
      vector: promptEmbedding.embedding,
      limit: k * 2,
      with_payload: true,
    });
    logger.info({ response }, "Recalled nodes from Qdrant");
    const results = response.filter((p) =>
      !recentlyRecalled.has(p.id.toString())
    ).map((point) => {
      recentlyRecalled.set(point.id.toString(), new Date());
      return point.payload;
    }).slice(0, k);
    logger.info({ results }, "Recalled nodes");
    recentlyRecalled.forEach((date, key) => {
      if (new Date().getTime() - date.getTime() > 1000 * 60 * 120) {
        recentlyRecalled.delete(key);
      }
    });
    return results;
  } catch (error) {
    logger.error("Error recalling nodes from Qdrant:", error);
    return [];
  }
}

// Initialize Qdrant Collection
// await initializeQdrantCollection();
export async function establishMemory() {
  const collectionExists = await qdrant.collectionExists(COLLECTION_NAME);
  if (!collectionExists) {
    await initializeQdrantCollection();
  }
}

export async function loadConversation() {
  // Load conversation from Neo4j
  const session = createSession();
  const result = await session.run(`
    MATCH (n:ChatMessage) RETURN n ORDER BY n.when DESC LIMIT 10
  `);
  const messages = result.records.map((record) => {
    const { role, content } = record.get("n").properties;
    return { role, content };
  });
  return messages;
}

export async function storeMessage(role: string, content: string) {
  const session = createSession();
  await session.run(
    `
    CREATE (n:ChatMessage { role: $role, content: $content, when: datetime() })
  `,
    { role, content },
  );
}

export async function latestSituation() {
  const session = createSession();
  const result = await session.run(`
    MATCH (n:Situation) RETURN n ORDER BY n.when DESC LIMIT 1
  `);
  if (result.records.length === 0) {
    return null;
  }
  const n = result.records[0].get("n").properties;
  return n;
}
