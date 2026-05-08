const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(script.includes('let audioStarted = false'));
assert(script.includes('if (audioStarted || ws.readyState !== WebSocket.OPEN)'));
assert(script.includes('const audioClipDurationMs = 500'));
assert(script.includes('const audioClipSamples = Math.round((targetSampleRate * audioClipDurationMs) / 1000)'));
assert(script.includes('while (queuedAudioSamples >= audioClipSamples)'));
assert(script.includes('at: capturedAt.toISOString()'));
assert(script.includes('setupAudio();'));
assert(script.includes('type: "SpeechPlayback"'));
assert(script.includes('reportPlayback("Started")'));
assert(script.includes('done("Finished")'));
assert(script.includes('done("Interrupted")'));
assert(!script.includes('if (navigator.mediaDevices?.getUserMedia) {\n    setupAudio();\n  }\n  setupSpeechRecognition();'));
console.log('audio-after-open ok');
