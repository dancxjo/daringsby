const WebSocket = require('ws');
const ws = new WebSocket('ws://localhost:3000/ws');
ws.on('open', () => {
    console.log('Connected to ws://localhost:3000/ws');
});
ws.on('message', (data) => {
    console.log('Received:', data.toString().substring(0, 200));
});
setTimeout(() => ws.close(), 5000);
