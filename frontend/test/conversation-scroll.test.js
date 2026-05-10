const assert = require('assert');

function updateConversation(log, msgs, container) {
  const isFirst = !log.textContent.trim();
  const wasAtBottom = container.scrollTop + container.clientHeight >= container.scrollHeight - 10;
  const prevScrollTop = container.scrollTop;

  log.textContent = msgs.join('\n');
  container.scrollHeight = log.textContent.length + 10; // simple mock

  if (wasAtBottom || isFirst) {
    container.scrollTop = container.scrollHeight;
  } else {
    container.scrollTop = prevScrollTop;
  }
}

// starts empty (first update)
const log0 = {textContent: ''};
const container0 = {scrollTop: 0, clientHeight: 0, scrollHeight: 0};
updateConversation(log0, ['a'], container0);
assert.strictEqual(container0.scrollTop, container0.scrollHeight, 'First update should scroll to bottom');

// at bottom
const log1 = {textContent: 'a'};
const container1 = {scrollTop: 80, clientHeight: 20, scrollHeight: 100};
updateConversation(log1, ['a', 'b', 'c'], container1);
assert.strictEqual(container1.scrollTop, container1.scrollHeight, 'Should scroll to new bottom if was at bottom');

// user scrolled up
const log2 = {textContent: 'a'};
const container2 = {scrollTop: 0, clientHeight: 20, scrollHeight: 100};
updateConversation(log2, ['a', 'b'], container2);
assert.strictEqual(container2.scrollTop, 0, 'Should preserve position if not at bottom');

console.log('ok');
