const assert = require('assert');

function updateConversation(log, msgs) {
  const atBottom =
    log.scrollTop + log.clientHeight >= log.scrollHeight - 5;
  log.textContent = msgs.join('\n');
  log.scrollHeight = log.textContent.length; // simple mock
  if (atBottom) {
    log.scrollTop = log.scrollHeight;
  }
}

// starts at bottom
const log = {scrollTop: 100, clientHeight: 20, scrollHeight: 100, textContent: ''};
updateConversation(log, ['a', 'b', 'c']);
assert.strictEqual(log.scrollTop, log.scrollHeight);

// user scrolled up
const log2 = {scrollTop: 0, clientHeight: 20, scrollHeight: 100, textContent: ''};
updateConversation(log2, ['a', 'b']);
assert.strictEqual(log2.scrollTop, 0);

console.log('ok');
