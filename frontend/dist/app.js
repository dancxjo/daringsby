(function () {
  const wsProtocol = location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${wsProtocol}//${location.host}/ws`);

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
  const swapCameraButton = document.getElementById("swap-camera");
  const player = document.getElementById("audio-player");
  const face = document.getElementById("face");
  const audioQueue = [];
  const conversationLog = document.getElementById("conversation-log");
  const typescriptSourceCode = document.getElementById("typescript-source-code");
  const typescriptResults = document.getElementById("typescript-results");
  const conversationMsgs = [];
  const witOutputs = {};
  const thoughtElems = {};
  const witDetails = {};
  const witDebugContainer = document.getElementById("wit-debug");
  let playing = false;
  let logAtBottom = true;
  let wordsAtBottom = true;
  let currentSpeechText = null;
  let currentSpeechKey = null;
  let resumeSpeechPlayback = null;

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
        case "Say": {
          const isFirst = !words.textContent.trim();
          const wasAtBottom = words.scrollTop + words.clientHeight >= words.scrollHeight - 10;
          const prevScrollTop = words.scrollTop;

          words.textContent += (words.textContent ? "\n" : "") + m.data.words;

          if (wasAtBottom || isFirst) {
            words.scrollTop = words.scrollHeight;
            wordsAtBottom = true;
          } else {
            words.scrollTop = prevScrollTop;
            wordsAtBottom = false;
          }
          enqueueAudio({ audio: m.data.audio || null, text: m.data.words });
          break;
        }
        case "Think":
          handleThink(m);
          break;
        case "Chunk":
          thought.textContent = m.data;
          break;
        case "SystemPrompt":
          setSystemPrompt(m.data);
          updateConversation();
          break;
        case "ConversationEntry":
          conversationMsgs.push(m.data);
          updateConversation();
          break;
        case "FullHistory":
          applyFullHistory(m.data);
          if (m.data.report) handleThink({ data: m.data.report });
          applyWillTypescript(m.data?.typescript || typescriptFromWillReport(m.data?.report));
          updateConversation();
          break;
      }
    } catch (e) {
      console.error(e);
    }
  }

  function enqueueAudio(item) {
    const key = speechQueueKey(item);
    if (key && (currentSpeechKey === key || audioQueue.some((queued) => queued.key === key))) {
      return;
    }
    audioQueue.push({ ...item, key });
    if (!playing) {
      playNext();
    }
  }

  function speechQueueKey(item) {
    return (item.text || "").trim();
  }

  function waitForSpeechGesture(resume) {
    if (resumeSpeechPlayback) return;
    resumeSpeechPlayback = () => {
      const fn = resumeSpeechPlayback;
      resumeSpeechPlayback = null;
      document.removeEventListener("pointerdown", fn);
      document.removeEventListener("keydown", fn);
      document.removeEventListener("click", fn);
      resume();
    };
    document.addEventListener("pointerdown", resumeSpeechPlayback, { once: true });
    document.addEventListener("keydown", resumeSpeechPlayback, { once: true });
    document.addEventListener("click", resumeSpeechPlayback, { once: true });
  }

  function playNext() {
    const next = audioQueue.shift();
    if (!next) {
      playing = false;
      currentSpeechText = null;
      currentSpeechKey = null;
      face.classList.remove("playing");
      startSpeechRecognition();
      return;
    }
    playing = true;
    currentSpeechText = next.text || null;
    currentSpeechKey = next.key || null;
    face.classList.add("playing");
    stopSpeechRecognition();

    let audioUrl = null;
    let started = false;
    let settled = false;
    let usingSpeechSynthesis = false;
    const reportPlayback = (status) => {
      if (!next.text) return;
      safeSend(JSON.stringify({
        type: "SpeechPlayback",
        text: next.text,
        status,
        at: new Date().toISOString(),
      }));
    };
    const cleanupAudioUrl = () => {
      if (audioUrl) {
        URL.revokeObjectURL(audioUrl);
        audioUrl = null;
      }
    };
    const done = (status) => {
      if (settled) return;
      settled = true;
      player.removeEventListener("ended", onEnded);
      player.removeEventListener("error", onError);
      player.removeEventListener("error", speakWithBrowserVoice);
      cleanupAudioUrl();
      if (!started && status === "Finished") {
        reportPlayback("Started");
      }
      reportPlayback(status);
      if (status === "Finished" && next.text) {
        safeSend(JSON.stringify({ type: "Echo", text: next.text, at: new Date().toISOString() }));
      }
      currentSpeechText = null;
      currentSpeechKey = null;
      playNext();
    };
    const onStarted = () => {
      if (started || settled) return;
      started = true;
      reportPlayback("Started");
    };
    const onEnded = () => done("Finished");
    const onError = () => done("Interrupted");
    const speakWithBrowserVoice = () => {
      if (settled || usingSpeechSynthesis) return;
      usingSpeechSynthesis = true;
      player.removeEventListener("ended", onEnded);
      player.removeEventListener("error", onError);
      cleanupAudioUrl();
      if ("speechSynthesis" in window && "SpeechSynthesisUtterance" in window) {
        const utterance = new SpeechSynthesisUtterance(next.text || "");
        utterance.onstart = onStarted;
        utterance.onend = onEnded;
        utterance.onerror = onError;
        window.speechSynthesis.speak(utterance);
      } else {
        onStarted();
        done("Interrupted");
      }
    };
    const tryAudioPlayback = () => {
      if (settled) return;
      player.play().then(onStarted).catch((err) => {
        if (err?.name === "NotAllowedError") {
          console.warn("audio autoplay blocked; speech remains queued", err);
          waitForSpeechGesture(tryAudioPlayback);
          return;
        }
        console.error("audio", err);
        speakWithBrowserVoice();
      });
    };

    if (next.audio) {
      audioUrl = audioObjectUrl(next.audio);
      player.src = audioUrl || `data:audio/wav;base64,${next.audio}`;
      player.addEventListener("ended", onEnded, { once: true });
      player.addEventListener("error", speakWithBrowserVoice, { once: true });
      tryAudioPlayback();
    } else if ("speechSynthesis" in window && "SpeechSynthesisUtterance" in window) {
      const utterance = new SpeechSynthesisUtterance(next.text || "");
      utterance.onstart = onStarted;
      utterance.onend = onEnded;
      utterance.onerror = onError;
      window.speechSynthesis.speak(utterance);
    } else {
      onStarted();
      done("Finished");
    }
  }

  function audioObjectUrl(base64) {
    if (!("Blob" in window) || !URL.createObjectURL) return null;
    try {
      const binary = atob(base64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i += 1) {
        bytes[i] = binary.charCodeAt(i);
      }
      return URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
    } catch (err) {
      console.warn("audio decode", err);
      return null;
    }
  }

  function captureWebcamFrame(video, canvas, ctx) {
    if (video.videoWidth === 0) {
      video.play().catch(() => { });
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
  ws.addEventListener("open", () => {
    webcamReady = true;
    if (navigator.mediaDevices?.getUserMedia) {
      setupWebcam();
      setupAudio();
    }
    setupBrowserMotion();
    startSpeechRecognition();
  });
  ws.addEventListener("close", () => {
    webcamReady = false;
  });

  document.getElementById("text-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const input = document.getElementById("text-input");
    const text = input.value.trim();
    if (text) {
      safeSend(JSON.stringify({ type: "Text", data: { text, at: new Date().toISOString() } }));
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
          at: new Date(pos.timestamp).toISOString(),
        })
      );
    });
  }

  let motionStarted = false;
  let motionPermissionRequested = false;
  let latestDeviceOrientation = null;
  let lastMotionSentAt = 0;
  const motionSendIntervalMs = 500;

  function setFiniteNumber(target, key, value) {
    if (typeof value === "number" && Number.isFinite(value)) {
      target[key] = value;
    }
  }

  function motionVector(reading) {
    if (!reading) return null;
    const out = {};
    setFiniteNumber(out, "x", reading.x);
    setFiniteNumber(out, "y", reading.y);
    setFiniteNumber(out, "z", reading.z);
    return Object.keys(out).length ? out : null;
  }

  function orientationVector(reading) {
    if (!reading) return null;
    const out = {};
    setFiniteNumber(out, "alpha", reading.alpha);
    setFiniteNumber(out, "beta", reading.beta);
    setFiniteNumber(out, "gamma", reading.gamma);
    if (typeof reading.absolute === "boolean") {
      out.absolute = reading.absolute;
    }
    return Object.keys(out).length ? out : null;
  }

  function sendBrowserMotion(data, at) {
    if (!data.acceleration &&
        !data.acceleration_including_gravity &&
        !data.rotation_rate &&
        !data.orientation) {
      return;
    }
    safeSend(
      JSON.stringify({
        type: "Motion",
        data,
        at,
      })
    );
  }

  function addBrowserMotionListeners() {
    if (motionStarted) return;
    motionStarted = true;
    if ("DeviceOrientationEvent" in window) {
      window.addEventListener("deviceorientation", (event) => {
        latestDeviceOrientation = orientationVector(event);
        const now = Date.now();
        if (!latestDeviceOrientation || now - lastMotionSentAt < motionSendIntervalMs) return;
        lastMotionSentAt = now;
        sendBrowserMotion(
          { orientation: latestDeviceOrientation },
          new Date(event.timeStamp ? performance.timeOrigin + event.timeStamp : now).toISOString()
        );
      });
    }
    if ("DeviceMotionEvent" in window) {
      window.addEventListener("devicemotion", (event) => {
        const now = Date.now();
        if (now - lastMotionSentAt < motionSendIntervalMs) return;
        lastMotionSentAt = now;
        const data = {
          acceleration: motionVector(event.acceleration),
          acceleration_including_gravity: motionVector(event.accelerationIncludingGravity),
          rotation_rate: orientationVector(event.rotationRate),
          orientation: latestDeviceOrientation,
        };
        setFiniteNumber(data, "interval", event.interval);
        sendBrowserMotion(
          data,
          new Date(event.timeStamp ? performance.timeOrigin + event.timeStamp : now).toISOString()
        );
      });
    }
  }

  function setupBrowserMotion() {
    if (!("DeviceMotionEvent" in window) && !("DeviceOrientationEvent" in window)) return;
    const needsMotionPermission =
      typeof DeviceMotionEvent !== "undefined" &&
      typeof DeviceMotionEvent.requestPermission === "function";
    const needsOrientationPermission =
      typeof DeviceOrientationEvent !== "undefined" &&
      typeof DeviceOrientationEvent.requestPermission === "function";
    if (!needsMotionPermission && !needsOrientationPermission) {
      addBrowserMotionListeners();
      return;
    }
    if (motionPermissionRequested) return;
    motionPermissionRequested = true;
    window.addEventListener(
      "pointerdown",
      async () => {
        try {
          const results = await Promise.all([
            needsMotionPermission ? DeviceMotionEvent.requestPermission() : Promise.resolve("granted"),
            needsOrientationPermission ? DeviceOrientationEvent.requestPermission() : Promise.resolve("granted"),
          ]);
          if (results.some((result) => result === "granted")) {
            addBrowserMotionListeners();
          }
        } catch (e) {
          console.warn("browser motion permission denied", e);
        }
      },
      { once: true }
    );
  }

  let webcamStream = null;
  let webcamReady = false;
  let webcamStarting = false;
  let webcamCaptureInterval = null;
  let selectedVideoDeviceId = null;
  let preferredFacingMode = null;
  let audioStarted = false;

  async function listVideoDevices() {
    if (!navigator.mediaDevices?.enumerateDevices) return [];
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices.filter((device) => device.kind === "videoinput" && device.deviceId);
  }

  function webcamVideoConstraints() {
    if (selectedVideoDeviceId) {
      return { deviceId: { exact: selectedVideoDeviceId } };
    }
    if (preferredFacingMode) {
      return { facingMode: { ideal: preferredFacingMode } };
    }
    return true;
  }

  function stopWebcamStream() {
    if (webcamStream) {
      webcamStream.getTracks().forEach((t) => t.stop());
      webcamStream = null;
    }
  }

  function resetWebcamCaptureLoop() {
    if (webcamCaptureInterval) {
      clearInterval(webcamCaptureInterval);
      webcamCaptureInterval = null;
    }
  }

  async function setupWebcam() {
    if (webcamStarting) return;
    webcamStarting = true;
    if (swapCameraButton) {
      swapCameraButton.disabled = true;
    }
    try {
      const video = document.getElementById("webcam");
      if (webcamStream?.active) {
        return; // already running
      }
      stopWebcamStream();
      console.debug("requesting webcam access");
      const stream = await navigator.mediaDevices.getUserMedia({ video: webcamVideoConstraints() });
      webcamStream = stream;
      stream.getTracks().forEach((t) =>
        t.addEventListener(
          "ended",
          () => {
            if (webcamReady) setupWebcam();
          },
          { once: true }
        )
      );
      console.debug("webcam stream acquired");
      video.srcObject = stream;
      await video.play();
      const canvas = document.createElement("canvas");
      canvas.id = "webcam-canvas";
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      resetWebcamCaptureLoop();
      webcamCaptureInterval = setInterval(() => {
        if (!webcamReady) return;
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
        safeSend(JSON.stringify({ type: "See", data, at: new Date().toISOString() }));
      }, 1000);
    } catch (e) {
      if (e?.name === "NotFoundError") {
        console.warn("webcam not available");
      } else {
        console.error("webcam", e);
      }
      mien.textContent = "🦯";
    } finally {
      webcamStarting = false;
      if (swapCameraButton) {
        swapCameraButton.disabled = false;
      }
    }
  }

  async function swapCamera() {
    if (!webcamReady || webcamStarting || !navigator.mediaDevices?.getUserMedia) return;
    if (swapCameraButton) {
      swapCameraButton.disabled = true;
    }
    try {
      const devices = await listVideoDevices();
      const currentDeviceId =
        webcamStream?.getVideoTracks()[0]?.getSettings?.().deviceId || selectedVideoDeviceId;

      if (devices.length > 1) {
        const currentIndex = devices.findIndex((device) => device.deviceId === currentDeviceId);
        selectedVideoDeviceId = devices[(currentIndex + 1 + devices.length) % devices.length].deviceId;
        preferredFacingMode = null;
      } else {
        selectedVideoDeviceId = null;
        preferredFacingMode = preferredFacingMode === "environment" ? "user" : "environment";
      }

      stopWebcamStream();
      resetWebcamCaptureLoop();
      await setupWebcam();
    } catch (e) {
      console.warn("camera swap", e);
      if (swapCameraButton) {
        swapCameraButton.disabled = false;
      }
    }
  }

  if (navigator.mediaDevices?.getUserMedia) {
    if (webcamReady) setupWebcam();
  } else if (swapCameraButton) {
    swapCameraButton.disabled = true;
  }

  if (swapCameraButton) {
    swapCameraButton.addEventListener("click", swapCamera);
  }

  async function setupAudio() {
    if (audioStarted || ws.readyState !== WebSocket.OPEN) {
      return;
    }
    audioStarted = true;
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const audioContext = new AudioContext();
      const source = audioContext.createMediaStreamSource(stream);
      const processor = audioContext.createScriptProcessor(4096, 1, 1);
      const targetSampleRate = 16000;
      const audioClipDurationMs = 500;
      const audioClipSamples = Math.round((targetSampleRate * audioClipDurationMs) / 1000);
      let queuedAudio = [];
      let queuedAudioSamples = 0;
      let queuedAudioStartedAt = null;

      window.onbeforeunload = () => {
        try {
          if (currentSpeechText) {
            safeSend(JSON.stringify({
              type: "SpeechPlayback",
              text: currentSpeechText,
              status: "Interrupted",
              at: new Date().toISOString(),
            }));
          }
          processor.disconnect();
          source.disconnect();
          audioContext.close();
          stream.getTracks().forEach((t) => t.stop());
        } catch (err) {
          console.warn("audio cleanup", err);
        }
      };

      processor.onaudioprocess = (event) => {
        if (playing) {
          queuedAudio = [];
          queuedAudioSamples = 0;
          queuedAudioStartedAt = null;
          return;
        }
        const input = event.inputBuffer.getChannelData(0);
        const pcm = floatTo16BitPcm(resample(input, audioContext.sampleRate, targetSampleRate));
        if (!pcm.byteLength) return;
        queueAudioClip(pcm);
      };

      function queueAudioClip(pcm) {
        if (!queuedAudioStartedAt) {
          queuedAudioStartedAt = new Date();
        }
        queuedAudio.push(pcm);
        queuedAudioSamples += pcm.length;

        while (queuedAudioSamples >= audioClipSamples) {
          const clip = takeQueuedAudioSamples(audioClipSamples);
          const capturedAt = queuedAudioStartedAt;
          safeSend(
            JSON.stringify({
              type: "Hear",
              data: {
                base64: arrayBufferToBase64(clip.buffer),
                mime: "audio/pcm;format=s16le;rate=16000",
                sample_rate: targetSampleRate,
                channels: 1,
              },
              at: capturedAt.toISOString(),
            })
          );
          queuedAudioStartedAt =
            queuedAudioSamples > 0
              ? new Date(capturedAt.getTime() + audioClipDurationMs)
              : null;
        }
      }

      function takeQueuedAudioSamples(sampleCount) {
        const clip = new Int16Array(sampleCount);
        let offset = 0;
        while (offset < sampleCount && queuedAudio.length) {
          const next = queuedAudio[0];
          const needed = sampleCount - offset;
          if (next.length <= needed) {
            clip.set(next, offset);
            offset += next.length;
            queuedAudio.shift();
          } else {
            clip.set(next.subarray(0, needed), offset);
            queuedAudio[0] = next.subarray(needed);
            offset += needed;
          }
        }
        queuedAudioSamples -= sampleCount;
        return clip;
      }

      source.connect(processor);
      processor.connect(audioContext.destination);
    } catch (e) {
      audioStarted = false;
      console.error("audio", e);
    }
  }

  function resample(input, fromRate, toRate) {
    if (fromRate === toRate) return input;
    const ratio = fromRate / toRate;
    const length = Math.floor(input.length / ratio);
    const output = new Float32Array(length);
    for (let i = 0; i < length; i += 1) {
      const pos = i * ratio;
      const before = Math.floor(pos);
      const after = Math.min(before + 1, input.length - 1);
      const weight = pos - before;
      output[i] = input[before] * (1 - weight) + input[after] * weight;
    }
    return output;
  }

  function floatTo16BitPcm(input) {
    const output = new Int16Array(input.length);
    for (let i = 0; i < input.length; i += 1) {
      const s = Math.max(-1, Math.min(1, input[i]));
      output[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
    }
    return output;
  }

  function arrayBufferToBase64(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i += 1) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  let startSpeechRecognition = () => {};
  let stopSpeechRecognition = () => {};

  function setupSpeechRecognition() {
    const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!SpeechRecognition) {
      console.warn("speech recognition unavailable; raw Hear frames will not produce text");
      return;
    }

    const recognition = new SpeechRecognition();
    recognition.continuous = true;
    recognition.interimResults = false;
    recognition.lang = navigator.language || "en-US";

    let active = false;
    const start = () => {
      if (
        active ||
        playing ||
        ws.readyState !== WebSocket.OPEN ||
        document.visibilityState === "hidden"
      ) {
        return;
      }
      try {
        recognition.start();
        active = true;
      } catch (err) {
        console.warn("speech recognition start", err);
      }
    };
    startSpeechRecognition = start;
    stopSpeechRecognition = () => {
      if (!active) return;
      try {
        recognition.stop();
      } catch (err) {
        console.warn("speech recognition stop", err);
      }
    };

    recognition.onresult = (event) => {
      for (let i = event.resultIndex; i < event.results.length; i += 1) {
        const result = event.results[i];
        if (!result.isFinal) continue;
        const transcript = result[0]?.transcript?.trim();
        if (transcript) {
          safeSend(JSON.stringify({ type: "Text", data: { text: transcript, at: new Date().toISOString() } }));
        }
      }
    };
    recognition.onerror = (event) => {
      active = false;
      console.warn("speech recognition", event.error || event);
    };
    recognition.onend = () => {
      active = false;
      setTimeout(start, 500);
    };
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden" && active) {
        recognition.stop();
      } else {
        start();
      }
    });
    startSpeechRecognition();
  }

  if (!navigator.mediaDevices?.getUserMedia) {
    setupSpeechRecognition();
  }

  function updateConversation() {
    const system = document.getElementById("system-prompt");
    if (system && conversationMsgs.length) {
      system.textContent = conversationMsgs[0].content;
    }
    const container = conversationLog.closest(".scroll-well") || conversationLog;
    const isFirst = !conversationLog.textContent.trim();
    const wasAtBottom = container.scrollTop + container.clientHeight >= container.scrollHeight - 10;
    const prevScrollTop = container.scrollTop;

    conversationLog.textContent = conversationMsgs
      .slice(1)
      .map((m) => {
        const ts = m.timestamp ? new Date(m.timestamp).toLocaleTimeString() + " " : "";
        return `${ts}${m.role}: ${m.content}`;
      })
      .join("\n");

    if (wasAtBottom || isFirst) {
      container.scrollTop = container.scrollHeight;
      logAtBottom = true;
    } else {
      container.scrollTop = prevScrollTop;
      logAtBottom = false;
    }
  }

  function setSystemPrompt(content) {
    const entry = { role: "system", content: content || "", timestamp: "" };
    if (conversationMsgs[0]?.role === "system") {
      conversationMsgs[0] = entry;
    } else {
      conversationMsgs.unshift(entry);
    }
  }

  function hasVisibleConversation() {
    return conversationMsgs.slice(1).some((m) => (m?.content || "").trim());
  }

  function applyFullHistory(data) {
    const history = Array.isArray(data?.history) ? data.history : [];
    setSystemPrompt(data?.system_prompt || conversationMsgs[0]?.content || "");
    if (history.length || !hasVisibleConversation()) {
      conversationMsgs.length = 1;
      conversationMsgs.push(...history);
    }
  }

  function applyWillTypescript(execution) {
    if (!typescriptSourceCode || !typescriptResults || !execution) return;
    const source = execution.source || "";
    typescriptSourceCode.textContent = source || "// no TypeScript selected";
    if (window.hljs?.highlightElement) {
      delete typescriptSourceCode.dataset.highlighted;
      window.hljs.highlightElement(typescriptSourceCode);
    }

    const results = Array.isArray(execution.results) ? execution.results : [];
    const timestamp = execution.timestamp ? new Date(execution.timestamp).toLocaleTimeString() : "";
    typescriptResults.textContent = [
      timestamp ? `at ${timestamp}` : "",
      ...results.map((result) => {
        const command = result?.command || "command";
        const output = result?.output || "";
        return `${command}: ${output}`;
      }),
    ].filter(Boolean).join("\n\n") || "waiting...";
  }

  function typescriptFromWillReport(report) {
    if (!report || report.name !== "Will" || typeof report.output !== "string") return null;
    const payload = parseWillReportJson(report.output);
    if (!payload || typeof payload.typescript !== "string") return null;
    return {
      source: payload.typescript,
      timestamp: new Date().toISOString(),
      results: [],
    };
  }

  function parseWillReportJson(raw) {
    try {
      return JSON.parse(raw);
    } catch (_) {
      const start = raw.indexOf("{");
      const end = raw.lastIndexOf("}");
      if (start === -1 || end <= start) return null;
      try {
        return JSON.parse(raw.slice(start, end + 1));
      } catch (_) {
        return null;
      }
    }
  }

  document.querySelectorAll("details").forEach(animateDetails);
})();
