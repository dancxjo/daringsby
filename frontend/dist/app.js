(function () {
  const wsProtocol = location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${wsProtocol}//${location.hostname}:3000/ws`);
  const debugWs = new WebSocket(`${wsProtocol}//${location.hostname}:3000/debug`);
  const mien = document.getElementById("mien");
  const words = document.getElementById("words");
  const thought = document.getElementById("thought");
  const thoughtTabs = document.getElementById("thought-tabs");
  const thoughtImage = document.getElementById("thought-image");
  const imageThumbnail = document.getElementById("image-thumbnail");
  const player = document.getElementById("audio-player");
  const audioQueue = [];
  const witOutputs = {};
  const witDetails = {};
  const witDebugContainer = document.getElementById("wit-debug");
  let playing = false;

  document.addEventListener("keydown", (e) => {
    if (e.altKey && e.code === "Backquote") {
      document.body.classList.toggle("retro-tabs");
    }
  });

  function getWitDetail(name) {
    let entry = witDetails[name];
    if (!entry) {
      const details = document.createElement("details");
      const summary = document.createElement("summary");
      const link = document.createElement("a");
      link.href = `/debug/wit/${name.toLowerCase()}`;
      link.target = "_blank";
      link.textContent = "link";
      summary.textContent = name + " ";
      summary.appendChild(link);
      const promptPre = document.createElement("pre");
      const outputPre = document.createElement("pre");
      details.appendChild(summary);
      details.appendChild(promptPre);
      details.appendChild(outputPre);
      witDebugContainer.appendChild(details);
      entry = { promptPre, outputPre };
      witDetails[name] = entry;
    }
    return entry;
  }

  function enqueueAudio(item) {
    audioQueue.push(item);
    if (!playing) {
      playNext();
    }
  }

  function playNext() {
    const next = audioQueue.shift();
    if (!next) {
      playing = false;
      return;
    }
    playing = true;

    const done = () => {
      player.removeEventListener("ended", done);
      player.removeEventListener("error", done);
      if (next.text) {
        ws.send(JSON.stringify({ type: "Echo", data: next.text }));
      }
      playNext();
    };

    if (next.audio) {
      player.src = `data:audio/wav;base64,${next.audio}`;
      player.addEventListener("ended", done, { once: true });
      player.addEventListener("error", done, { once: true });
      player.play().catch((err) => {
        console.error("audio", err);
        done();
      });
    } else {
      done();
    }
  }

  function handleMessage(ev) {
    try {
      const m = JSON.parse(ev.data);
      switch (m.type) {
        case "Emote":
        case "emote":
          mien.textContent = m.data;
          break;
        case "Say":
        case "say":
          words.textContent += "\n" + m.data.words;
          words.scrollTop = words.scrollHeight;
          enqueueAudio({ audio: m.data.audio || null, text: m.data.words });
          break;
        case "Think":
        case "think": {
          if (typeof m.data === "object" && m.data !== null) {
            witOutputs[m.data.name] = m.data.output;
            const { promptPre, outputPre } = getWitDetail(m.data.name);
            if (m.data.prompt !== undefined) {
              promptPre.textContent = m.data.prompt;
            }
            if (m.data.output !== undefined) {
              outputPre.textContent = m.data.output;
            }
          } else {
            witOutputs["unknown"] = m.data;
          }
          thoughtTabs.innerHTML = "";
          Object.entries(witOutputs).forEach(([name, output]) => {
            const div = document.createElement("div");
            div.className = "wit-report";
            div.textContent = `${name}: ${output}`;
            thoughtTabs.appendChild(div);
          });
          thought.style.display = Object.keys(witOutputs).length ? "flex" : "none";
          break;
        }
        case "Heard":
        case "heard":
          // ignore for now
          break;
      }
    } catch (e) {
      console.error(e);
    }
  }

  ws.onmessage = handleMessage;
  debugWs.onmessage = handleMessage;

  document.getElementById("text-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const input = document.getElementById("text-input");
    const text = input.value.trim();
    if (text) {
      ws.send(JSON.stringify({ type: "Text", data: text }));
      input.value = "";
    }
  });

  if (navigator.geolocation) {
    navigator.geolocation.watchPosition((pos) => {
      ws.send(
        JSON.stringify({
          type: "Geolocate",
          data: {
            longitude: pos.coords.longitude,
            latitude: pos.coords.latitude,
          },
        }),
      );
    });
  }

  async function setupWebcam() {
    try {
      const video = document.getElementById("webcam");
      const stream = await navigator.mediaDevices.getUserMedia({ video: true });
      video.srcObject = stream;
      await video.play();
      const canvas = document.createElement("canvas");
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      setInterval(() => {
        if (video.videoWidth === 0) {
          video.play().catch(() => {});
          return;
        }
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        ctx.drawImage(video, 0, 0);
        const pixel = ctx.getImageData(
          Math.floor(canvas.width / 2),
          Math.floor(canvas.height / 2),
          1,
          1,
        ).data;
        const blank = pixel[0] === 0 && pixel[1] === 0 && pixel[2] === 0;
        if (!blank) {
          const data = canvas.toDataURL("image/jpeg");
          thoughtImage.src = data;
          thoughtImage.style.display = "block";
          imageThumbnail.src = data;
          imageThumbnail.style.display = "block";
          ws.send(JSON.stringify({ type: "See", data: data }));
        } else {
          ws.send(JSON.stringify({ type: "See", data: "" }));
        }
      }, 1000);
    } catch (e) {
      if (e && e.name === "NotFoundError") {
        console.warn("webcam not available");
      } else {
        console.error("webcam", e);
      }
    }
  }

  if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
    setupWebcam();
  }

  async function setupAudio() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const rec = new MediaRecorder(stream);
      rec.ondataavailable = (e) => {
        if (e.data.size > 0) {
          const reader = new FileReader();
          reader.onloadend = () => {
            const base64 = reader.result.split(",")[1];
            ws.send(
              JSON.stringify({
                type: "Hear",
                data: { base64: base64, mime: e.data.type },
              }),
            );
          };
          reader.readAsDataURL(e.data);
        }
      };
      rec.start(1000);
    } catch (e) {
      console.error("audio", e);
    }
  }

  if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
    setupAudio();
  }

  async function updateConversation() {
    try {
      const resp = await fetch("/conversation");
      const msgs = await resp.json();
      document.getElementById("conversation-log").textContent = msgs
        .map((m) => `${m.role}: ${m.content}`)
        .join("\n");
    } catch (e) {
      console.error("conversation", e);
    }
  }

  setInterval(updateConversation, 2000);
  updateConversation();
})();
