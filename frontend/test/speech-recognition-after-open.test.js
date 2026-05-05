const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('let startSpeechRecognition = () => {};'));
assert(script.includes('startSpeechRecognition();'));
assert(script.includes('startSpeechRecognition = start;'));
assert(script.includes('active = false;\n      console.warn("speech recognition"'));
console.log('speech-recognition-after-open ok');
