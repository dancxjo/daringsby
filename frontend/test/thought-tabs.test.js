const assert = require('assert');
const { JSDOM } = require('jsdom');
const dom = new JSDOM('<div id="tabs"></div>');
global.window = dom.window;
global.document = dom.window.document;
const { updateThoughtTabs } = require('../dist/thoughtTabs.js');

const container = document.getElementById('tabs');
const map = {};

updateThoughtTabs(container, {Quick: 'ok'}, map);
assert.strictEqual(container.children.length, 1);
const first = container.children[0];
assert.strictEqual(first.textContent, 'Quick: ok');

updateThoughtTabs(container, {Quick: 'done'}, map);
assert.strictEqual(container.children.length, 1);
assert.strictEqual(container.children[0], first);
assert.strictEqual(first.textContent, 'Quick: done');

updateThoughtTabs(container, {}, map);
assert.strictEqual(container.children.length, 0);
console.log('ok');
