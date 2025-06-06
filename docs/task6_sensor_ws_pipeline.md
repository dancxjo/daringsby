# Task 6: Sensor to Sensation WebSocket Pipeline

This task introduces a simple WebSocket server that converts browser sensor events into `Sensation` structs. A small dev panel is served from `/devpanel` and streams geolocation and text input to the backend.

Run the server:
```bash
cargo run -p sensation-server
```
Then open [http://localhost:8000/devpanel](http://localhost:8000/devpanel).

A CLI tester is also available:
```bash
cargo run -p sensation-tester -- --geo 45.0 122.0
```
