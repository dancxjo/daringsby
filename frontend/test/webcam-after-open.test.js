const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('let webcamReady = false'));
assert(script.includes('if (!webcamReady) return'));
console.log('webcam-after-open ok');
