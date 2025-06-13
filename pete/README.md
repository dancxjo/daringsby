# pete

A simple Deno module exposing sensory primitives powered by RxJS.

```ts
import { Sensor } from "./mod.ts";

const sensor = new Sensor<string>((s) => s.what.length > 0);

sensor.subscribe((s) => console.log(`felt ${s.what} at ${s.when}`));

sensor.feel("warmth");
```

Run tests with `deno test` (ensure you have Deno installed).

