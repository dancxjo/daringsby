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
  app.$nextTick = (cb) => cb();
  app.ws = { send: jest.fn() };
  return app;
}

function loadAppWithSocket() {
  const html = fs.readFileSync(path.join(__dirname, '../..', 'index.html'), 'utf8');
  const dom = new JSDOM(html, { runScripts: 'dangerously' });
  const { window } = dom;
  global.document = window.document;
  global.window = window;
  window.navigator.mediaDevices = { getUserMedia: () => Promise.resolve({}) };
  const socket = { send: jest.fn() };
  window.WebSocket = jest.fn(() => socket);
  const app = window.chatApp();
  app.$refs = {
    log: document.createElement('div'),
    player: { src: '', play: jest.fn(() => Promise.resolve()), onended: null },
    video: {}
  };
  app.$nextTick = (cb) => cb();
  app.connect();
  return { app, socket };
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

test('speech event appends text and queues audio', () => {
  const { app, socket } = loadAppWithSocket();
  socket.onmessage({
    data: JSON.stringify({ kind: 'pete-speech', text: 'hello', audio: 'AA==' })
  });
  expect(app.log[0].text).toBe('hello');
  expect(app.playing).toBe(true);
  expect(app.$refs.player.src).toBe('data:audio/wav;base64,AA==');
  // queue is drained immediately
  expect(app.audioQueue.length).toBe(0);
  // No display ack is sent; playback ack happens later.
  expect(socket.send).not.toHaveBeenCalled();
});
