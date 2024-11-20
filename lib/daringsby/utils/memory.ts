import neo4j from "npm:neo4j-driver";
import { Config, EmbeddingsResponse, Ollama } from "npm:ollama";
import { mergeMap, Observable, OperatorFunction } from "npm:rxjs";

const driver = neo4j.driver(
    Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
    neo4j.auth.basic("neo4j", "password"),
);
const session = driver.session();

interface Document<T = unknown> {
    metadata: {
        label: string;
    };
    data: T;
}

type Embedding = EmbeddingsResponse;

/**
 * Establishes the central node representing Pete Daringsby in the graph database.
 */
export async function establishMemory(): Promise<void> {
    await memorize({
        metadata: {
            label: "Self",
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
            components: ["Heart", "Wits", "FondDuCoeur", "SocketConnection"],
            creator: "Travis Reed",
            languages: ["English", "French"],
            sensoryModules: [
                "vision",
                "geolocation",
                "text input",
                "echo perception",
            ],
            personalityTraits: ["curious", "introspective", "cautious"],
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
}

/**
 * Stores a document as a node in the graph database, including a linked embedding node.
 */
export async function memorize<T = unknown>(
    document: Document<T>,
): Promise<void> {
    try {
        const url = Deno.env.get("OLLAMA_URL") || "http://localhost:11434";
        const ollama = new Ollama({
            host: url,
        });
        const embedding = await ollama.embeddings({
            prompt: JSON.stringify(document.data),
            model: "nomic-embed-text",
        });

        // Create the document node
        const docQuery = `
            CREATE (doc:${document.metadata.label} {
                data: $data
            })
            RETURN doc
        `;
        const docResult = await session.run(docQuery, {
            data: JSON.stringify(document.data),
        });
        const docNode = docResult.records[0].get("doc");

        // Create the embedding node and link it to the document node
        const embeddingQuery = `
            CREATE (embed:Embedding {
                vector: $embedding
            })
            WITH embed
            MATCH (doc)
            WHERE id(doc) = $docId
            CREATE (doc)-[:HAS_EMBEDDING]->(embed)
        `;
        await session.run(embeddingQuery, {
            embedding: embedding.embedding,
            docId: docNode.identity.toNumber(),
        });
    } catch (error) {
        console.error("Error embedding document: ", error);
    }
}

/**
 * Queries the graph database for nodes containing specific context.
 */
export async function queryMemory(context: string): Promise<any[]> {
    const result = await session.run(context);
    return result.records.map((record) => record.get("n"));
}

/**
 * Recalls the top k nodes from the graph database based on a given prompt.
 */
export async function recall(prompt: string, k: number = 10): Promise<any[]> {
    try {
        const ollama = new Ollama();
        const promptEmbedding: EmbeddingsResponse = await ollama.embeddings({
            prompt,
            model: "nomic-embed-text",
        });
        const allEmbeddingsQuery = `
            MATCH (doc)-[:HAS_EMBEDDING]->(embed:Embedding)
            RETURN doc, embed.vector AS embedding
        `;
        const result = await session.run(allEmbeddingsQuery);
        const nodes = result.records.map((record) => ({
            node: record.get("doc"),
            embedding: record.get("embedding"),
        }));
        const neighbors = findNearestNeighbors(
            promptEmbedding.embedding,
            nodes.map((n) => n.embedding),
            k,
        );
        return neighbors.map((neighbor) => nodes[neighbor.index].node);
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
