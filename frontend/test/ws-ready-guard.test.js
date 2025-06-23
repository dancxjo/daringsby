const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('function waitForWebSocketReady()'));
assert(script.includes('readyState === WebSocket.OPEN'));
console.log('ws-ready-guard ok');
