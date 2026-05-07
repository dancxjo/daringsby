const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(script.includes('const maxEmbeddingLinksPerCluster = 80;'));
assert(script.includes('const syntheticRelationships = ['));
assert(script.includes('...embeddingNeighborRelationships(graph.nodes, fullGraph.relationships)'));
assert(script.includes('...semanticSimilarityRelationships(graph.nodes, fullGraph.relationships, fullGraph.nodes)'));
assert(script.includes('function embeddingNeighborRelationships(nodes, relationships)'));
assert(script.includes('function semanticSimilarityRelationships(nodes, relationships, contextNodes = nodes)'));
assert(script.includes('function similarityRelationshipsForVectorClusters(nodes, relationships, options)'));
assert(script.includes('function vectorOwnersByKind(nodes, relationships)'));
assert(script.includes('rel.type !== "HAS_CLUSTER_MEMBER" && rel.type !== "MEMBER_OF_CLUSTER"'));
assert(script.includes('type: "SIMILAR_EMBEDDING"'));
assert(script.includes('type: "SIMILAR_FACE"'));
assert(script.includes('type: "SIMILAR_VOICE_SIGNATURE"'));
assert(script.includes('synthetic: true'));
assert(script.includes('display_only: true'));
assert(script.includes('function linkDistance(link)'));
assert(script.includes('function linkStrokeWidth(link)'));
assert(script.includes('function similarityStrength(link)'));
assert(styles.includes('.embedding-link'));
assert(styles.includes('.face-similarity-link'));
assert(styles.includes('.voice-signature-similarity-link'));
console.log('psychic-embedding-links ok');
