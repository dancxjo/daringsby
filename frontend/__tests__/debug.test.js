const fs = require('fs');
const path = require('path');
const { JSDOM } = require('jsdom');

function loadApp() {
  const html = fs.readFileSync(path.join(__dirname, '../..', 'index.html'), 'utf8');
  const dom = new JSDOM(html, { runScripts: 'dangerously' });
  const { window } = dom;
  global.document = window.document;
  global.window = window;
  window.navigator.mediaDevices = { getUserMedia: () => Promise.resolve({}) };
  const socket = {};
  window.WebSocket = jest.fn(() => socket);
  const app = window.chatApp();
  app.$refs = { log: document.createElement('div'), player: {}, video: {} };
  app.$nextTick = (cb) => cb();
  app.ws = { send: jest.fn() };
  app.connectDebug();
  return { app, socket };
}

test('debug messages append thoughts', () => {
  const { app, socket } = loadApp();
  const msg = JSON.stringify({ type: 'Think', data: 'Will: go => ok' });
  socket.onmessage({ data: msg });
  expect(app.thoughts[0]).toBe('Will: go => ok');
});
