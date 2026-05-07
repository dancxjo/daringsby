const assert = require('assert');
const fs = require('fs');

const script = fs.readFileSync('frontend/psychic/psychic.js', 'utf8');
const styles = fs.readFileSync('frontend/psychic/styles.css', 'utf8');

assert(script.includes('function renderRelationshipLinks(container, relationships)'));
assert(script.includes('props.relationships = node.relationships.map((rel) => relationshipReferenceForNode(node.id, rel));'));
assert(script.includes('link.href = graphTargetHref(target);'));
assert(script.includes('navigateToGraphTarget(target, { updateUrl: true })'));
assert(script.includes('function targetFromLocation()'));
assert(script.includes('const relationshipId = params.get("relationship") || "";'));
assert(script.includes('window.history[options.replaceUrl ? "replaceState" : "pushState"]({}, "", next);'));
assert(script.includes('function snapRelationshipIntoView(rel)'));
assert(script.includes('snapPointsIntoView('));
assert(script.includes('window.addEventListener("popstate"'));
assert(styles.includes('.relationship-list'));
assert(styles.includes('.relationship-list a:hover'));
console.log('psychic-relationship-links ok');
