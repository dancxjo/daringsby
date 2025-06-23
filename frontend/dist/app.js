(function () {
  const wsProtocol = location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${wsProtocol}//${location.hostname}:3000/ws`);
  const debugWs = new WebSocket(`${wsProtocol}//${location.hostname}:3000/debug`);

  function waitForWebSocketReady() {
    if (ws.readyState === WebSocket.OPEN) return Promise.resolve();
    return new Promise((resolve) => {
      const handle = () => {
        if (ws.readyState === WebSocket.OPEN) {
          clearInterval(interval);
          resolve();
        }
      };
      const interval = setInterval(handle, 50);
      ws.addEventListener("open", handle, { once: true });
    });
  }

  function safeSend(data) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(data);
    } else {
      console.warn("WebSocket not ready; dropping message");
    }
  }
  const mien = document.getElementById("mien");
  const words = document.getElementById("words");
  const thought = document.getElementById("thought");
  const thoughtTabs = document.getElementById("thought-tabs");
  const thoughtImage = document.getElementById("thought-image");
  const imageThumbnail = document.getElementById("image-thumbnail");
  const player = document.getElementById("audio-player");
  const face = document.getElementById("face");
  const audioQueue = [];
  const conversationLog = document.getElementById("conversation-log");
  const witOutputs = {};
  const thoughtElems = {};
  const witDetails = {};
  const witDebugContainer = document.getElementById("wit-debug");
  let playing = false;
  let debugMode = false;

  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey && e.key === "d") {
      debugMode = !debugMode;
      updateConversation();
    }
  });

  function animateDetails(details) {
    const summary = details.querySelector("summary");
    if (!summary) return;
    const collapsed = summary.offsetHeight;
    if (!details.hasAttribute("open")) {
      details.style.maxHeight = collapsed + "px";
    }
    summary.addEventListener("click", (e) => {
      e.preventDefault();
      const open = details.hasAttribute("open");
      const start = details.scrollHeight;
      details.style.maxHeight = start + "px";
      details.style.overflow = "hidden";
      requestAnimationFrame(() => {
        details.style.transition = "max-height 0.2s ease";
        details.style.maxHeight = open ? collapsed + "px" : details.scrollHeight + "px";
      });
      details.addEventListener("transitionend", () => {
        details.style.removeProperty("transition");
        if (open) {
          details.removeAttribute("open");
          details.style.maxHeight = collapsed + "px";
        } else {
          details.style.maxHeight = "none";
        }
      }, { once: true });
      if (!open) {
        details.setAttribute("open", "");
      }
    });
  }

  async function fetchWits() {
    try {
      const resp = await fetch("/debug/psyche");
      const info = await resp.json();
      (info.active_wits || []).forEach((name) => {
        const entry = getWitDetail(name);
        if (info.last_ticks && info.last_ticks[name]) {
          entry.time.textContent = new Date(info.last_ticks[name]).toLocaleTimeString();
        }
      });
    } catch (e) {
      console.error("wits", e);
    }
  }

  fetchWits();
  setInterval(fetchWits, 5000);

  function getWitDetail(name) {
    let entry = witDetails[name];
    if (!entry) {
      const details = document.createElement("details");
      details.id = `wit-${name}-details`;
      details.setAttribute("data-wit-name", name);

      const summary = document.createElement("summary");
      summary.id = `wit-${name}-summary`;

      const link = document.createElement("a");
      link.id = `wit-${name}-debug-link`;
      link.href = `/debug/wit/${name.toLowerCase()}`;
      link.target = "_blank";
      link.textContent = "link";

      const time = document.createElement("span");
      time.id = `wit-${name}-time`;
      time.className = "wit-time";

      summary.textContent = name + " ";
      summary.appendChild(time);
      summary.appendChild(link);

      const promptPre = document.createElement("pre");
      promptPre.id = `wit-${name}-prompt`;
      const outputPre = document.createElement("pre");
      outputPre.id = `wit-${name}-output`;
      outputPre.textContent = "waiting...";

      details.appendChild(summary);
      details.appendChild(promptPre);
      details.appendChild(outputPre);

      animateDetails(details);
      witDebugContainer.appendChild(details);

      entry = { promptPre, outputPre, time, details };
      witDetails[name] = entry;
    }
    return entry;
  }

  function handleThink(m) {
    if (typeof m.data === "object" && m.data !== null) {
      witOutputs[m.data.name] = m.data.output;
      const { promptPre, outputPre, time, details } = getWitDetail(m.data.name);
      if (m.data.prompt !== undefined) {
        promptPre.textContent = m.data.prompt;
      }
      if (m.data.output !== undefined) {
        outputPre.textContent = JSON.stringify(m.data.output, null, 2);
      }
      time.textContent = new Date().toLocaleTimeString();
      details.classList.add("updated");
      setTimeout(() => details.classList.remove("updated"), 300);
    } else {
      witOutputs["unknown"] = m.data;
    }

    thoughtTabs.innerHTML = "";
    Object.entries(witOutputs).forEach(([name, output]) => {
      const div = document.createElement("div");
      div.className = "wit-report";
      div.id = `wit-report-${name}`;
      div.textContent = `${name}: ${output}`;
      thoughtTabs.appendChild(div);
    });

    thought.style.display = Object.keys(witOutputs).length ? "flex" : "none";
  }

  function handleMainMessage(ev) {
    try {
      const m = JSON.parse(ev.data);
      switch (m.type) {
        case "Emote":
          mien.textContent = m.data;
          break;
        case "Say":
          words.textContent += "\n" + m.data.words;
          words.scrollTop = words.scrollHeight;
          enqueueAudio({ audio: m.data.audio || null, text: m.data.words });
          break;
        case "Think":
          handleThink(m);
          break;
      }
    } catch (e) {
      console.error(e);
    }
  }

  function handleDebugMessage(ev) {
    try {
      const m = JSON.parse(ev.data);
      if (m.type === "Think") {
        handleThink(m);
      }
    } catch (e) {
      console.error(e);
    }
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
      face.classList.remove("playing");
      return;
    }
    playing = true;
    face.classList.add("playing");

    const done = () => {
      player.removeEventListener("ended", done);
      player.removeEventListener("error", done);
      if (next.text) {
        safeSend(JSON.stringify({ type: "Echo", text: next.text }));
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

  function captureWebcamFrame(video, canvas, ctx) {
    if (video.videoWidth === 0) {
      video.play().catch(() => {});
      return null;
    }
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    ctx.drawImage(video, 0, 0);
    const pixel = ctx.getImageData(canvas.width / 2, canvas.height / 2, 1, 1).data;
    const blank = pixel[0] === 0 && pixel[1] === 0 && pixel[2] === 0;
    return blank ? "" : canvas.toDataURL("image/jpeg");
  }

  ws.onmessage = handleMainMessage;
  debugWs.onmessage = handleDebugMessage;

  document.getElementById("text-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const input = document.getElementById("text-input");
    const text = input.value.trim();
    if (text) {
      safeSend(JSON.stringify({ type: "Text", text }));
      input.value = "";
    }
  });

  if (navigator.geolocation) {
    navigator.geolocation.watchPosition((pos) => {
      safeSend(
        JSON.stringify({
          type: "Geolocate",
          data: {
            longitude: pos.coords.longitude,
            latitude: pos.coords.latitude,
          },
        })
      );
    });
  }

  let webcamStream = null;

  async function setupWebcam() {
    try {
      const video = document.getElementById("webcam");
      if (webcamStream) {
        webcamStream.getTracks().forEach((t) => t.stop());
      }
      console.log("requesting webcam access");
      const stream = await navigator.mediaDevices.getUserMedia({ video: true });
      webcamStream = stream;
      stream.getTracks().forEach((t) =>
        t.addEventListener(
          "ended",
          () => waitForWebSocketReady().then(setupWebcam),
          { once: true }
        )
      );
      console.log("webcam stream acquired");
      video.srcObject = stream;
      await video.play();
      const canvas = document.createElement("canvas");
      canvas.id = "webcam-canvas";
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      setInterval(() => {
        const data = captureWebcamFrame(video, canvas, ctx);
        if (data === null) return;
        if (data) {
          thoughtImage.src = data;
          thoughtImage.style.display = "block";
          imageThumbnail.src = data;
          imageThumbnail.style.display = "block";
        } else {
          thoughtImage.style.display = "none";
          imageThumbnail.style.display = "none";
        }
        safeSend(JSON.stringify({ type: "See", data }));
      }, 1000);
    } catch (e) {
      if (e?.name === "NotFoundError") {
        console.warn("webcam not available");
      } else {
        console.error("webcam", e);
      }
      mien.textContent = "🚫 Webcam unavailable";
    }
  }

  if (navigator.mediaDevices?.getUserMedia) {
    waitForWebSocketReady().then(setupWebcam);
  }

  async function setupAudio() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const rec = new MediaRecorder(stream);
      window.onbeforeunload = () => {
        try {
          if (rec.state !== "inactive") rec.stop();
          stream.getTracks().forEach((t) => t.stop());
        } catch (err) {
          console.warn("recorder cleanup", err);
        }
      };
      rec.ondataavailable = (e) => {
        if (e.data.size > 0) {
          const reader = new FileReader();
          reader.onloadend = () => {
            const base64 = reader.result.split(",")[1];
            safeSend(
              JSON.stringify({
                type: "Hear",
                data: { base64: base64, mime: e.data.type },
              })
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

  if (navigator.mediaDevices?.getUserMedia) {
    setupAudio();
  }

  async function updateConversation() {
    try {
      const resp = await fetch(`/conversation${debugMode ? "?debug=1" : ""}`);
      const msgs = await resp.json();
      const system = document.getElementById("system-prompt");
      if (system && msgs.length) {
        system.textContent = msgs[0].content;
      }
      const atBottom =
        conversationLog.scrollTop + conversationLog.clientHeight >=
        conversationLog.scrollHeight - 5;
      conversationLog.textContent = msgs
        .slice(1)
        .map((m) => {
          const ts =
            debugMode && m.timestamp
              ? new Date(m.timestamp).toLocaleTimeString() + " "
              : "";
          return `${ts}${m.role}: ${m.content}`;
        })
        .join("\n");
      if (atBottom) {
        conversationLog.scrollTop = conversationLog.scrollHeight;
      }
    } catch (e) {
      console.error("conversation", e);
    }
  }

  setInterval(updateConversation, 2000);
  updateConversation();
  document.querySelectorAll("details").forEach(animateDetails);
})();
