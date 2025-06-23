// Update or remove DOM nodes for wit outputs
// Usage: updateThoughtTabs(container, outputs, map)
function updateThoughtTabs(container, outputs, elements) {
  for (const [name, output] of Object.entries(outputs)) {
    let node = elements[name];
    if (!node) {
      node = document.createElement('div');
      node.className = 'wit-report';
      elements[name] = node;
      container.appendChild(node);
    }
    node.textContent = `${name}: ${output}`;
  }
  for (const name of Object.keys(elements)) {
    if (!(name in outputs)) {
      elements[name].remove();
      delete elements[name];
    }
  }
}
if (typeof module !== 'undefined') {
  module.exports = { updateThoughtTabs };
} else {
  window.updateThoughtTabs = updateThoughtTabs;
}

