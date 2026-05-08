const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(script.includes('selectItem({ kind: "node", value: node }, { playMedia: true });'));
assert(script.includes('clip.addEventListener("click", (event) => selectTimelineClip(item, event));'));
assert(script.includes('el.addEventListener("click", () => selectItem({ kind: "node", value: segment.node }, { playMedia: true }));'));
assert(script.includes('renderMediaPreview(node, { autoplay: options.playMedia === true });'));
assert(script.includes('const playableMedia = nodeHasPlayableMedia(node);'));
assert(script.includes('loadNodeDetails(node, { preserveMediaPreview: playableMedia, autoplayMedia: options.playMedia === true });'));
assert(script.includes('preserveExisting: options.preserveMediaPreview,'));
assert(script.includes('autoplay: options.autoplayMedia === true,'));
assert(script.includes('preview.dataset.speechSegmentId = node.id;'));
assert(script.includes('preview.dataset.audioClipId = node.id;'));
assert(script.includes('preview.dataset.mediaNodeId = node.id;'));
assert(script.includes('preview.src = audioClipAudioSrc(node);'));
assert(script.includes('function audioClipAudioSrc(node)'));
assert(script.includes('return `/graph/audio-clip/${encodeURIComponent(node.id)}/audio.wav`;'));
assert(script.includes('preview.play().catch(() => {});'));
assert(script.includes('function existingSpeechSegmentPreview(id)'));
assert(script.includes('function existingMediaPreview(node)'));
assert(script.includes('function nodeHasPlayableMedia(node)'));
assert(script.includes('if (nodeKind(node) === "AudioClip") return true;'));
assert(styles.includes('.present-speech-segment:focus-visible'));
assert(styles.includes('pointer-events: auto;'));

console.log('psychic-speech-segment-playback ok');
