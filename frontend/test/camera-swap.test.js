const assert = require('assert');
const fs = require('fs');

const html = fs.readFileSync('frontend/dist/index.html', 'utf8');
const script = fs.readFileSync('frontend/dist/app.js', 'utf8');

assert(html.includes('id="swap-camera"'));
assert(script.includes('const swapCameraButton = document.getElementById("swap-camera")'));
assert(script.includes('function webcamVideoConstraints()'));
assert(script.includes('function stopWebcamStream()'));
assert(script.includes('function resetWebcamCaptureLoop()'));
assert(script.includes('let webcamStarting = false'));
assert(script.includes('if (webcamStarting) return'));
assert(script.includes('async function swapCamera()'));
assert(script.includes('navigator.mediaDevices.enumerateDevices()'));
assert(script.includes('swapCameraButton.addEventListener("click", swapCamera)'));
console.log('camera-swap ok');
