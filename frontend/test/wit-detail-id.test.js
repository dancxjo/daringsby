const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');

assert(script.includes('id = `wit-${name}-details`'));
assert(script.includes('id = `wit-${name}-summary`'));
assert(script.includes('id = `wit-${name}-debug-link`'));
assert(script.includes('id = `wit-${name}-time`'));
assert(script.includes('id = `wit-${name}-prompt`'));
assert(script.includes('id = `wit-${name}-output`'));
assert(script.includes('div.id = `wit-report-${name}`'));
assert(script.includes('canvas.id = "webcam-canvas"'));
console.log('wit-detail-id ok');
