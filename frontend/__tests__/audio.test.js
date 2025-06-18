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
  const app = window.chatApp();
  app.$refs = { player: { src: '', play: jest.fn(() => Promise.resolve()), onended: null }, log: document.createElement('div'), video: {} };
  app.ws = { send: jest.fn() };
  return app;
}

test('playNext loads audio and sends ack', () => {
  const app = loadApp();
  app.audioQueue.push({ audio: 'UklGRg==', text: 'hi' });
  app.playNext();
  expect(app.$refs.player.src).toBe('data:audio/wav;base64,UklGRg==');
  expect(app.playing).toBe(true);
  app.$refs.player.onended();
  expect(app.ws.send).toHaveBeenCalledWith(JSON.stringify({ type: 'played', text: 'hi' }));
  expect(app.playing).toBe(false);
});
