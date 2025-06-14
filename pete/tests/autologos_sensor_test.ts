import { Autologos } from "../../sensors/autologos.ts";

function assert(condition: boolean, msg: string) {
  if (!condition) throw new Error(msg);
}

Deno.test("fileTree lists repository files", async () => {
  const sensor = new Autologos(0, 0, ".");
  sensor.stop();
  const tree = await (sensor as any).fileTree();
  assert(tree.includes("sensors/"), "expected sensors directory");
});

Deno.test("codeSection returns snippet", async () => {
  const sensor = new Autologos(0, 0, "sensors");
  sensor.stop();
  const section = await (sensor as any).codeSection();
  assert(section.includes("export"), "expected snippet with code");
});

Deno.test("stateInfo reports memory", () => {
  const sensor = new Autologos();
  sensor.stop();
  const info = (sensor as any).stateInfo();
  assert(info.includes("rss="), "expected rss in state info");
});
