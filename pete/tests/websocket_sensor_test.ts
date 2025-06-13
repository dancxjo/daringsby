import { WebSocketSensor } from "../../sensors/websocket.ts";
import { assertEquals } from "https://deno.land/std@0.200.0/testing/asserts.ts";

Deno.test("connected emits connection experience", () => {
  const sensor = new WebSocketSensor();
  let type = "";
  sensor.subscribe((exp) => {
    type = exp.what[0].what.type;
  });
  sensor.connected("ip");
  assertEquals(type, "connect");
});

Deno.test("received emits message experience", () => {
  const sensor = new WebSocketSensor();
  let received = "";
  sensor.subscribe((exp) => {
    if (exp.what[0].what.type === "message") {
      received = (exp.what[0].what as any).message;
    }
  });
  sensor.received("ip", "hi");
  assertEquals(received, "hi");
});
