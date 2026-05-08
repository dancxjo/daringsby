const assert = require('assert');
const fs = require('fs');

const styles = fs.readFileSync('frontend/dist/styles.css', 'utf8');

assert(styles.includes('@media (max-width: 900px)'));
assert(styles.includes('flex-direction: column'));
assert(styles.includes('min-height: 100svh'));
assert(styles.includes('grid-template-columns: auto auto minmax(0, 1fr)'));
assert(styles.includes('max-width: none'));
assert(styles.includes('@media (max-width: 520px)'));
console.log('mobile-layout ok');
