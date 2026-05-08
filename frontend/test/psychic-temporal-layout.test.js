const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');

assert(script.includes('const temporalMarginRatio = 0.12;'));
assert(script.includes('const temporalLayoutPropertyKeys = ['));
assert(script.includes('let lastTemporalSignature = "";'));
assert(script.includes('.force("time-x", d3.forceX(temporalX).strength(temporalXStrength))'));
assert(script.includes('const temporalSignature = signatureForTemporalLayout(fullGraph);'));
assert(script.includes('applyGraphFilters(topologyChanged || temporalChanged);'));
assert(script.includes('function updateTemporalExtent()'));
assert(script.includes('function temporalX(node)'));
assert(script.includes('function nodeTimestamp(node)'));
assert(script.includes('function temporalLayoutKeys(node)'));
assert(script.includes('"source_started_at",'));
assert(script.includes('"source_captured_at",'));
assert(script.includes('"source_ended_at",'));
assert(!script.includes('"transcribed_at",'));
assert(script.includes('if (typeof value === "number" && Number.isFinite(value)) return value;'));
assert(script.includes('return left + clamp01(ratio) * (right - left);'));
assert(script.includes('simulation.force("time-x").x(temporalX);'));
assert(script.includes('!temporalProperty(key) || temporalLayoutKey(key)'));
assert(script.includes('function signatureForTemporalLayout(snapshot)'));
console.log('psychic-temporal-layout ok');
