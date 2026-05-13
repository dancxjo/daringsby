const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');

assert(script.includes('const temporalLayoutPropertyKeys = ['));
assert(script.includes('let lastTemporalSignature = "";'));
assert(!script.includes('.force("time-x"'));
assert(script.includes('const temporalSignature = signatureForTemporalLayout(fullGraph);'));
assert(script.includes('applyGraphFilters(topologyChanged);'));
assert(script.includes('function nodeTimestamp(node)'));
assert(script.includes('"source_started_at",'));
assert(script.includes('"source_captured_at",'));
assert(script.includes('"source_ended_at",'));
assert(!script.includes('"transcribed_at",'));
assert(script.includes('if (typeof value === "number" && Number.isFinite(value)) return value;'));
assert(script.includes('!temporalProperty(key) || temporalLayoutKey(key)'));
assert(script.includes('function signatureForTemporalLayout(snapshot)'));
console.log('psychic-temporal-layout ok');
