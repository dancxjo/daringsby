const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('SpeechRecognition'));
assert(script.includes('webkitSpeechRecognition'));
assert(script.includes('type: "Text", data: { text: transcript, at: new Date().toISOString() }'));
console.log('speech-recognition-text ok');
