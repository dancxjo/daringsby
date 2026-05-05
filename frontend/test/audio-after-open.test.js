const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('let audioStarted = false'));
assert(script.includes('if (audioStarted || ws.readyState !== WebSocket.OPEN)'));
assert(script.includes('setupAudio();'));
assert(!script.includes('if (navigator.mediaDevices?.getUserMedia) {\n    setupAudio();\n  }\n  setupSpeechRecognition();'));
console.log('audio-after-open ok');
