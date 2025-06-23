const assert = require('assert');
const fs = require('fs');
const { JSDOM } = require('jsdom');

const html = fs.readFileSync('frontend/dist/index.html', 'utf8');
const dom = new JSDOM(html);
const details = dom.window.document.querySelector('details');
assert.strictEqual(details.getAttribute('data-wit-name'), 'conversation');

const appJs = fs.readFileSync('frontend/dist/app.js', 'utf8');
assert(appJs.includes('data-wit-name'), 'app.js should set data-wit-name attribute');

console.log('ok');
