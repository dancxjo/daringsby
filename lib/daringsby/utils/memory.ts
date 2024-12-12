import { logger } from "../core/logger.ts";
import neo4j, { RecordShape } from "npm:neo4j-driver";
import { Config, EmbeddingsResponse, Ollama } from "npm:ollama";
import { mergeMap, Observable, OperatorFunction } from "npm:rxjs";

const driver = neo4j.driver(
  Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
  neo4j.auth.basic("neo4j", "password"),
);
function createSession() {
  return driver.session({ defaultAccessMode: neo4j.session.WRITE });
}

/**
 * Reloads embeddings from the graph database.
 */
async function reloadEmbeddings(): Promise<void> {
  const session = createSession();
  try {
    const result = await session.run(`
            MATCH (doc)-[:HAS_EMBEDDING]->(embed:Embedding)
            RETURN doc, embed.vector AS embedding
        `);
    const nodes = result.records.map((record) => ({
      node: record.get("doc"),
      embedding: record.get("embedding"),
    }));
    logger.debug({ nodes }, "Reloaded nodes and embeddings");
    // Index the loaded embeddings for nearest neighbor searches
    indexEmbeddings(nodes);
  } catch (error) {
    console.error("Error reloading embeddings: ", error);
  } finally {
    await session.close();
  }
}

interface Document<T = unknown> {
  metadata: {
    label: string;
    when: string;
  };
  data: T;
}

type Embedding = EmbeddingsResponse;

let indexedEmbeddings: { node: any; embedding: number[] }[] = [];

function indexEmbeddings(nodes: { node: any; embedding: number[] }[]): void {
  indexedEmbeddings = nodes;
}

/**
 * Establishes the central node representing Pete Daringsby in the graph database.
 */
export async function establishMemory(): Promise<void> {
  const session = createSession();
  const tx = session.beginTransaction();
  try {
    // Delete all existing :Self nodes
    await tx.run(`
            MATCH (n:Self)
            DETACH DELETE n
        `);

    // Create or merge the new :Self node
    await memorize({
      metadata: {
        label: "Self",
        when: new Date().toISOString(),
      },
      data: {
        name: "Pete Daringsby",
        type: "Artificial Consciousness",
        description:
          "Pete is an artificial consciousness developed by Travis, serving as a learning and self-improving agent with various modules like Wits for introspection and a Heart for sensory data integration.",
        version: "Daringsby",
        environment: "Seattle, Washington, currently hosted on a laptop",
        functions: [
          "learning",
          "self-improvement",
          "speech recognition",
          "emotional interpretation",
          "contextual understanding",
        ],
        components: [
          "Heart",
          "Wits",
          "FondDuCoeur",
          "SocketConnection",
        ],
        creator: "Travis Reed",
        personalityTraits: [
          "curious",
          "introspective",
          "cautious",
          "friendly",
          "resourceful",
          "insightful",
          "honest",
        ],
        sensoryModules: [
          "vision",
          "geolocation",
          "text input",
          "echo perception",
        ],
        goals: [
          "provide coherent narratives",
          "maintain awareness",
          "improve memory management",
        ],
        proceduralMemory:
          "Stores routines, tasks, and procedural experiences like speech synthesis and interaction patterns.",
        autobiographicalMemory:
          "Graph database for long-term memory, recording key experiences and facts.",
        externalInterfaces: [
          "WebSocket for communication",
          "Graph database for memory",
        ],
        narrativeSystem:
          "Summarization system generating ongoing introspections, connected to Wits and Heart for processing experiences.",
        currentFocus:
          "Balancing sensory workload and refining response processing using RxJS",
      },
    });

    await tx.commit();

    // Reload embeddings
    await reloadEmbeddings();
  } catch (error) {
    await tx.rollback();
    console.error("Error establishing memory: ", error);
    throw error;
  } finally {
    await session.close();
  }
}

export async function runCypher(query: string): Promise<RecordShape> {
  const session = createSession();
  try {
    return await session.run(query);
  } finally {
    await session.close();
  }
}

/**
 * Stores a document as a node in the graph database, including a linked embedding node.
 */
export async function memorize<T = unknown>(
  document: Document<T>,
): Promise<void> {
  if (!document || !document.data) {
    return;
  }
  try {
    const url = Deno.env.get("OLLAMA_URL") || "http://localhost:11434";
    const ollama = new Ollama({
      host: url,
    });

    // Generate the embedding
    const embedding = await ollama.embeddings({
      prompt: JSON.stringify(document.data),
      model: "nomic-embed-text",
    });

    // Create Neo4j session and transaction
    const sessionInstance = createSession();
    const tx = sessionInstance.beginTransaction();

    // Log the document being stored
    logger.debug({ document }, "Storing document");

    try {
      // Dynamically create Cypher properties string
      const properties = Object.keys(document.data)
        .map((key) => `${key}: $${key}`)
        .join(", ");

      // Build the Cypher query
      const docQuery = `
                CREATE (doc:${document.metadata.label} { ${properties} })
                RETURN doc
            `;

      // Run the query with the flattened data as parameters
      const docResult = await tx.run(docQuery, document.data);

      // Retrieve the created node
      const docNode = docResult.records.length > 0
        ? docResult.records[0].get("doc")
        : null;

      if (docNode) {
        // Create the embedding node and link it to the document node
        const embeddingQuery = `
                    MERGE (embed:Embedding {
                        vector: $embedding
                    })
                    WITH embed
                    MATCH (doc)
                    WHERE id(doc) = $docId
                    MERGE (doc)-[:HAS_EMBEDDING]->(embed)
                `;
        await tx.run(embeddingQuery, {
          embedding: embedding.embedding,
          docId: docNode.identity.toNumber(),
        });
      }

      // Commit the transaction
      await tx.commit();
    } catch (error) {
      await tx.rollback();
      console.error("Error storing document: ", error);
      throw error;
    } finally {
      await sessionInstance.close();
    }
  } catch (error) {
    console.error("Error embedding document: ", error);
  }
}

/**
 * Queries the graph database for nodes containing specific context.
 */
export async function queryMemory(context: string): Promise<any[]> {
  const session = createSession();
  try {
    const result = await session.run(context);
    return result.records.map((record) => {
      const node = record.toObject();
      return {
        labels: node.labels,
        properties: node.properties,
      };
    });
  } finally {
    await session.close();
  }
}

/**
 * Recalls the top k nodes from the graph database based on a given prompt.
 */
export async function recall(prompt: string, k: number = 10): Promise<any[]> {
  if (!prompt) {
    return [];
  }
  logger.debug({ prompt }, "Recalling nodes");
  try {
    const ollama = new Ollama({
      host: Deno.env.get("OLLAMA_URL") || "http://localhost:11434",
    });
    const promptEmbedding: EmbeddingsResponse = await ollama.embeddings({
      prompt,
      model: "nomic-embed-text",
    });
    logger.debug("Got prompt embedding");
    const neighbors = findNearestNeighbors(
      promptEmbedding.embedding,
      indexedEmbeddings.map((n) => n.embedding),
      k,
    );
    return neighbors.map((neighbor) => indexedEmbeddings[neighbor.index].node);
  } catch (error) {
    console.error("Error recalling nodes: ", error);
    return [];
  }
}

/**
 * Generates embeddings for input strings using the Ollama API and returns them as an observable.
 */
export function embedify(
  model: string = "nomic-embed-text",
  config: Partial<Config> = {},
): OperatorFunction<string, Embedding> {
  const ollama = new Ollama(config);
  return (source: Observable<string>) =>
    source.pipe(
      mergeMap((input) => {
        return new Observable<Embedding>((observer) => {
          ollama.embeddings({
            prompt: input,
            model,
          }).then((embeddingResponse) => {
            observer.next(embeddingResponse);
            observer.complete();
          }).catch((error) => {
            observer.error(error);
          });
        });
      }),
    );
}

function findNearestNeighbors(
  embedding: number[],
  embeddings: number[][],
  k: number,
): { index: number; distance: number }[] {
  const distances = embeddings.map((e, index) => ({
    index,
    distance: cosineSimilarity(embedding, e),
  }));
  distances.sort((a, b) => b.distance - a.distance);
  return distances.slice(0, k);
}

function cosineSimilarity(a: number[], b: number[]): number {
  const dotProduct = a.reduce((sum, val, i) => sum + val * b[i], 0);
  const magnitudeA = Math.sqrt(a.reduce((sum, val) => sum + val * val, 0));
  const magnitudeB = Math.sqrt(b.reduce((sum, val) => sum + val * val, 0));
  return dotProduct / (magnitudeA * magnitudeB);
}

establishMemory();
