import { WebSocketSensor } from "../../sensors/websocket.ts";

function assertEquals(actual: unknown, expected: unknown) {
  if (actual !== expected) {
    throw new Error(`Expected ${expected}, got ${actual}`);
  }
}

Deno.test("connected emits connection experience", () => {
  const sensor = new WebSocketSensor();
  let type = "";
  sensor.subscribe((exp) => {
    type = exp.what[0].what.type;
  });
  sensor.connected("ip");
  assertEquals(type, "connect");
});

Deno.test("received emits message experience with name", () => {
  const sensor = new WebSocketSensor();
  let event;
  sensor.subscribe((exp) => {
    if (exp.what[0].what.type === "message") {
      event = exp;
    }
  });
  sensor.received("ip", "Bob", "hi");
  assertEquals(event!.what[0].what.message, "hi");
  assertEquals(event!.what[0].what.name, "Bob");
});

Deno.test("how uses provided name", () => {
  const sensor = new WebSocketSensor();
  let how = "";
  sensor.subscribe((exp) => {
    if (exp.what[0].what.type === "message") {
      how = exp.how;
    }
  });
  sensor.received("ip", "Alice", "hello");
  assertEquals(how, "Alice says: hello");
});

Deno.test("self emits internal desire", () => {
  const sensor = new WebSocketSensor();
  let type = "";
  sensor.subscribe((exp) => type = exp.what[0].what.type);
  sensor.self("hi");
  assertEquals(type, "self");
});

Deno.test("echoed emits echo event", () => {
  const sensor = new WebSocketSensor();
  let type = "";
  sensor.subscribe((exp) => type = exp.what[0].what.type);
  sensor.echoed("ip", "ok");
  assertEquals(type, "echo");
});
