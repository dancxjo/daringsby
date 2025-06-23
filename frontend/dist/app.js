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
  const face = document.getElementById("face");
  const audioQueue = [];
  const conversationLog = document.getElementById("conversation-log");
  const witOutputs = {};
  const witDetails = {};
  const witDebugContainer = document.getElementById("wit-debug");
  let playing = false;

  function animateDetails(details) {
    const summary = details.querySelector("summary");
    if (!summary) return;
    summary.addEventListener("click", (e) => {
      e.preventDefault();
      const open = details.hasAttribute("open");
      const startHeight = details.offsetHeight;
      const summaryHeight = summary.offsetHeight;
      details.style.height = startHeight + "px";
      details.style.overflow = "hidden";
      requestAnimationFrame(() => {
        details.style.transition = "height 0.2s ease";
        details.style.height = open ? summaryHeight + "px" : details.scrollHeight + "px";
      });
      details.addEventListener(
        "transitionend",
        () => {
          details.style.removeProperty("height");
          details.style.removeProperty("overflow");
          details.style.removeProperty("transition");
          if (open) {
            details.removeAttribute("open");
          }
        },
        { once: true }
      );
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
      const summary = document.createElement("summary");
      const link = document.createElement("a");
      link.href = `/debug/wit/${name.toLowerCase()}`;
      link.target = "_blank";
      link.textContent = "link";
      const time = document.createElement("span");
      time.className = "wit-time";
      summary.textContent = name + " ";
      summary.appendChild(time);
      summary.appendChild(link);
      const promptPre = document.createElement("pre");
      const outputPre = document.createElement("pre");
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
        ws.send(JSON.stringify({ type: "Echo", text: next.text }));
      }
      playNext();
      if (!playing) {
        face.classList.remove("playing");
      }
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
      }
    } catch (e) {
      console.error(e);
    }
  }

  function handleDebugMessage(ev) {
    try {
      const m = JSON.parse(ev.data);
      if (m.type === "Think") {
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
          div.textContent = `${name}: ${output}`;
          thoughtTabs.appendChild(div);
        });
        thought.style.display = Object.keys(witOutputs).length ? "flex" : "none";
      }
    } catch (e) {
      console.error(e);
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
    const pixel = ctx.getImageData(
      Math.floor(canvas.width / 2),
      Math.floor(canvas.height / 2),
      1,
      1,
    ).data;
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
      ws.send(JSON.stringify({ type: "Text", text }));
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
          const data = captureWebcamFrame(video, canvas, ctx);
          if (data === null) {
            return;
          }
          if (data) {
            thoughtImage.src = data;
            thoughtImage.style.display = "block";
            imageThumbnail.src = data;
            imageThumbnail.style.display = "block";
          } else {
            thoughtImage.style.display = "none";
            imageThumbnail.style.display = "none";
          }
          ws.send(JSON.stringify({ type: "See", data }));
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
      const system = document.getElementById("system-prompt");
      if (system && msgs.length) {
        system.textContent = msgs[0].content;
      }
      const atBottom =
        conversationLog.scrollTop + conversationLog.clientHeight >=
        conversationLog.scrollHeight - 5;
      conversationLog.textContent = msgs
        .slice(1)
        .map((m) => `${m.role}: ${m.content}`)
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
