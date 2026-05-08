const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(script.includes('selectItem({ kind: "node", value: node }, { playMedia: true });'));
assert(script.includes('clip.addEventListener("click", () => selectItem({ kind: "node", value: item.node }, { playMedia: true }));'));
assert(script.includes('el.addEventListener("click", () => selectItem({ kind: "node", value: segment.node }, { playMedia: true }));'));
assert(script.includes('renderMediaPreview(node, { autoplay: options.playMedia === true });'));
assert(script.includes('loadNodeDetails(node, { preserveMediaPreview: speechSegment });'));
assert(script.includes('renderMediaPreview(selected.value, { preserveExisting: options.preserveMediaPreview });'));
assert(script.includes('preview.dataset.speechSegmentId = node.id;'));
assert(script.includes('preview.play().catch(() => {});'));
assert(script.includes('function existingSpeechSegmentPreview(id)'));
assert(styles.includes('.present-speech-segment:focus-visible'));
assert(styles.includes('pointer-events: auto;'));

console.log('psychic-speech-segment-playback ok');
