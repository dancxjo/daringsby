const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('🚫 Webcam unavailable'));
console.log('webcam-error ok');
