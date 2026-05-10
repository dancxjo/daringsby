const assert = require('assert');

function updateConversation(log, msgs, container) {
  const atBottom =
    container.scrollTop + container.clientHeight >= container.scrollHeight - 10;
  log.textContent = msgs.join('\n');
  container.scrollHeight = log.textContent.length + 10; // simple mock (10 for label)
  if (atBottom) {
    container.scrollTop = container.scrollHeight;
  }
}

// starts at bottom
const log = {textContent: ''};
const container = {scrollTop: 90, clientHeight: 20, scrollHeight: 100};
updateConversation(log, ['a', 'b', 'c'], container);
assert.strictEqual(container.scrollTop, container.scrollHeight);

// user scrolled up
const log2 = {textContent: ''};
const container2 = {scrollTop: 0, clientHeight: 20, scrollHeight: 100};
updateConversation(log2, ['a', 'b'], container2);
assert.strictEqual(container2.scrollTop, 0);

console.log('ok');
