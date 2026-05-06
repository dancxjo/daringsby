const assert = require('assert');
const fs = require('fs');

const html = fs.readFileSync('frontend/psychic/index.html', 'utf8');
const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(html.includes('id="graph-filters"'));
assert(html.includes('id="label-filters"'));
assert(html.includes('id="predicate-filters"'));
assert(script.includes('const filters = {'));
assert(script.includes('labels: new Map()'));
assert(script.includes('predicates: new Map()'));
assert(script.includes('const filterStorageKey = "psychic.graph.filters.v1";'));
assert(script.includes('loadStoredFilters();'));
assert(script.includes('function syncFilterControls()'));
assert(script.includes('function applyGraphFilters(reheat = false)'));
assert(script.includes('graph.nodes = fullGraph.nodes.filter(nodeMatchesLabelFilters);'));
assert(script.includes('visibleNodeIds.has(source) && visibleNodeIds.has(target) && predicateAllowed(rel.type)'));
assert(script.includes('filterGroup(kind).set(value, input.checked);'));
assert(script.includes('saveStoredFilters();'));
assert(script.includes('function loadStoredFilters()'));
assert(script.includes('window.localStorage.getItem(filterStorageKey)'));
assert(script.includes('function saveStoredFilters()'));
assert(script.includes('window.localStorage.setItem('));
assert(script.includes('function filterControlId(kind, value)'));
assert(styles.includes('.filter-panel'));
assert(styles.includes('.filter-option input'));
console.log('psychic-filters ok');
