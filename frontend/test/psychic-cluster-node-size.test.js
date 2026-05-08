const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(script.includes('if (hasNodeLabel(node, "Cluster")) return 31;'));
assert(script.includes('if (kind === "Face" || kind === "Voice") return kind;'));
assert(script.includes('if (nodeKind(node) === "Theme") return 35;'));
assert(script.includes('if (isEmbeddingNode(node)) return 5;'));
assert(script.includes('.classed("embedding-node", isEmbeddingNode)'));
assert(script.includes('.attr("dy", (node) => `${nodeRadius(node) + 10}px`)'));
assert(styles.includes('.embedding-node circle'));
assert(styles.includes('.embedding-node .node-icon'));
assert(script.includes('.strength(linkStrength)'));
assert(script.includes('function themeCenterStrength(node)'));
assert(script.includes('return nodeKind(node) === "Theme" ? 0.18 : 0.015;'));
console.log('psychic-cluster-node-size ok');
