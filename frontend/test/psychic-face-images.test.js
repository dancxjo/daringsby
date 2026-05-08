const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(script.includes('renderFaceImageList(dd, value);'));
assert(script.includes('function renderFaceImageList(container, faceImages)'));
assert(script.includes('list.className = "face-image-list";'));
assert(script.includes('frame.className = "face-image-link";'));
assert(script.includes('image.src = dataUrl(mime, base64);'));
assert(script.includes('navigateToGraphTarget(target, { updateUrl: true })'));
assert(script.includes('function faceImageCaption(faceImage)'));
assert(script.includes('key === "face_images" ? compactCachedFaceImages(value) : value'));
assert(script.includes('function compactCachedFaceImages(value)'));
assert(script.includes('const { base64, ...compact } = faceImage;'));
assert(script.includes('nodeKind(node) === "Face" && Array.isArray(node.properties?.face_images)'));
assert(styles.includes('.face-image-list'));
assert(styles.includes('.face-image-link img'));
assert(styles.includes('grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));'));

console.log('psychic-face-images ok');
