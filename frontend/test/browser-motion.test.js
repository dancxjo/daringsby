const assert = require("assert");
const fs = require("fs");

const script = fs.readFileSync("frontend/dist/app.js", "utf8");

assert(script.includes("setupBrowserMotion();"));
assert(script.includes('"DeviceMotionEvent" in window'));
assert(script.includes('"deviceorientation"'));
assert(script.includes('"devicemotion"'));
assert(script.includes('type: "Motion"'));
assert(script.includes("acceleration_including_gravity"));
assert(script.includes("rotation_rate"));
assert(script.includes("motionSendIntervalMs = 500"));
console.log("browser-motion ok");
