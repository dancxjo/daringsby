const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');

assert(script.includes('if (nodeKind(node) === "Cluster") return 31;'));
assert(script.includes('.attr("dy", (node) => `${nodeRadius(node) + 10}px`)'));
console.log('psychic-cluster-node-size ok');
