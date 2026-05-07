const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');

assert(script.includes('const graphStore = {'));
assert(script.includes('nodes: new Map()'));
assert(script.includes('relationships: new Map()'));
assert(script.includes('const graphCacheDbName = "psychic.graph.cache.v1";'));
assert(script.includes('restoreGraphCache().catch(() => {'));
assert(script.includes('function openGraphCacheDb()'));
assert(script.includes('window.indexedDB.open(graphCacheDbName, graphCacheDbVersion)'));
assert(script.includes('db.createObjectStore("nodes", { keyPath: "id" });'));
assert(script.includes('db.createObjectStore("relationships", { keyPath: "id" });'));
assert(script.includes('function mergeGraphSnapshot(snapshot, options = {})'));
assert(script.includes('changed = mergeGraphNode(node, { persist: false }) || changed;'));
assert(script.includes('function materializeFullGraph()'));
assert(script.includes('fullGraph.nodes = [...graphStore.nodes.values()];'));
assert(script.includes('function scheduleGraphCacheSave()'));
assert(script.includes('function saveGraphCache()'));
assert(script.includes('graphStore.nodes.forEach((node) => nodeStore.put(serializeCachedNode(node)));'));
assert(script.includes('function cachedNodeDetails(id)'));
assert(script.includes('if (cached && (!options.requireComplete || cachedNodeDetailsComplete(cached))) return cached;'));
assert(script.includes('function cachedNodeDetailsComplete(node)'));
assert(script.includes('fetchNodeDetails(node.id, { requireComplete: true })'));
assert(script.includes('fetch(`/graph/node/${encodeURIComponent(id)}`, { cache: "force-cache" })'));
console.log('psychic-browser-cache ok');
