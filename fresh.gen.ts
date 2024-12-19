// DO NOT EDIT. This file is generated by Fresh.
// This file SHOULD be checked into source version control.
// This file is automatically updated during development when running `dev.ts`.

import * as $_404 from "./routes/_404.tsx";
import * as $_app from "./routes/_app.tsx";
import * as $chat from "./routes/chat.tsx";
import * as $index from "./routes/index.tsx";
import * as $senses_location from "./routes/senses/location.ts";
import * as $socket from "./routes/socket.ts";
import * as $AudioQueue from "./islands/AudioQueue.tsx";
import * as $Face from "./islands/Face.tsx";
import * as $Geolocator from "./islands/Geolocator.tsx";
import * as $LiveConnection from "./islands/LiveConnection.tsx";
import * as $Mien from "./islands/Mien.tsx";
import * as $SpokenWords from "./islands/SpokenWords.tsx";
import * as $TextInput from "./islands/TextInput.tsx";
import * as $ThoughtBubble from "./islands/ThoughtBubble.tsx";
import * as $Webcam from "./islands/Webcam.tsx";
import type { Manifest } from "$fresh/server.ts";

const manifest = {
  routes: {
    "./routes/_404.tsx": $_404,
    "./routes/_app.tsx": $_app,
    "./routes/chat.tsx": $chat,
    "./routes/index.tsx": $index,
    "./routes/senses/location.ts": $senses_location,
    "./routes/socket.ts": $socket,
  },
  islands: {
    "./islands/AudioQueue.tsx": $AudioQueue,
    "./islands/Face.tsx": $Face,
    "./islands/Geolocator.tsx": $Geolocator,
    "./islands/LiveConnection.tsx": $LiveConnection,
    "./islands/Mien.tsx": $Mien,
    "./islands/SpokenWords.tsx": $SpokenWords,
    "./islands/TextInput.tsx": $TextInput,
    "./islands/ThoughtBubble.tsx": $ThoughtBubble,
    "./islands/Webcam.tsx": $Webcam,
  },
  baseUrl: import.meta.url,
} satisfies Manifest;

export default manifest;
