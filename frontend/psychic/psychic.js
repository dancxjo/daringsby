(function () {
  if (!window.d3) {
    document.getElementById("status").textContent = "D3 failed to load";
    return;
  }

  const svg = d3.select("#graph");
  const root = svg.append("g");
  const linkLayer = root.append("g").attr("class", "links");
  const labelLayer = root.append("g").attr("class", "link-labels");
  const nodeLayer = root.append("g").attr("class", "nodes");
  const statusEl = document.getElementById("status");
  const nodeCountEl = document.getElementById("node-count");
  const relationshipCountEl = document.getElementById("relationship-count");
  const inspectorEmpty = document.getElementById("inspector-empty");
  const inspectorContent = document.getElementById("inspector-content");
  const inspectorIcon = document.getElementById("inspector-icon");
  const inspectorTitle = document.getElementById("inspector-title");
  const inspectorKind = document.getElementById("inspector-kind");
  const inspectorMedia = document.getElementById("inspector-media");
  const inspectorProperties = document.getElementById("inspector-properties");

  const palette = {
    Image: { color: "#63d2ff", icon: "◉" },
    AudioClip: { color: "#ffd166", icon: "♪" },
    Transcription: { color: "#f78fb3", icon: "T" },
    SpeechSegment: { color: "#f8a06b", icon: "§" },
    Sensation: { color: "#8ee6a8", icon: "S" },
    Impression: { color: "#b8a1ff", icon: "I" },
    Vector: { color: "#81ecec", icon: "V" },
    Geolocation: { color: "#a3e635", icon: "⌖" },
    Heartbeat: { color: "#ff7a90", icon: "♥" },
    ObjectObservation: { color: "#d2b48c", icon: "O" },
    Face: { color: "#ffcf99", icon: "◐" },
    Voice: { color: "#7dd3fc", icon: "V" },
    Person: { color: "#f0abfc", icon: "P" },
    RawPayload: { color: "#b8c0cc", icon: "R" },
    default: { color: "#c9d4de", icon: "◎" },
  };

  const fullGraph = { nodes: [], relationships: [] };
  const graph = { nodes: [], relationships: [] };
  const nodeState = new Map();
  let selected = null;
  let socket = null;
  let lastSnapshotSignature = "";
  let lastTopologySignature = "";
  let detailRequestId = 0;
  let mediaObjectUrl = "";

  const zoom = d3
    .zoom()
    .scaleExtent([0.15, 4])
    .on("zoom", (event) => root.attr("transform", event.transform));

  svg.call(zoom);

  const simulation = d3
    .forceSimulation()
    .force(
      "link",
      d3
        .forceLink()
        .id((node) => node.id)
        .distance((link) => 90 + Math.min(link.type.length * 2, 70))
        .strength(0.45),
    )
    .force("charge", d3.forceManyBody().strength(-520))
    .force("center", d3.forceCenter())
    .force("collision", d3.forceCollide().radius((node) => nodeRadius(node) + 9))
    .on("tick", ticked);

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    socket = new WebSocket(`${protocol}//${location.host}/ws`);
    statusEl.textContent = "Connecting";

    socket.addEventListener("open", () => {
      statusEl.textContent = "Live";
    });

    socket.addEventListener("message", (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch (_err) {
        return;
      }
      if (message.type === "GraphSnapshot") {
        updateGraph(message.data || { nodes: [], relationships: [] });
      } else if (message.type === "Error") {
        statusEl.textContent = message.data?.message || "Graph unavailable";
      }
    });

    socket.addEventListener("close", () => {
      statusEl.textContent = "Reconnecting";
      window.setTimeout(connect, 1200);
    });
  }

  function updateGraph(snapshot) {
    const snapshotSignature = signatureForSnapshot(snapshot);
    if (snapshotSignature === lastSnapshotSignature) {
      statusEl.textContent = "Live";
      return;
    }
    lastSnapshotSignature = snapshotSignature;

    const topologySignature = signatureForTopology(snapshot);
    const topologyChanged = topologySignature !== lastTopologySignature;
    lastTopologySignature = topologySignature;

    const previous = new Map(fullGraph.nodes.map((node) => [node.id, node]));
    fullGraph.nodes = (snapshot.nodes || []).map((node) => {
      const old = previous.get(node.id) || nodeState.get(node.id);
      const next = { ...node };
      if (old) {
        next.x = old.x;
        next.y = old.y;
        next.vx = old.vx;
        next.vy = old.vy;
        next.fx = old.fx;
        next.fy = old.fy;
      }
      nodeState.set(next.id, next);
      return next;
    });
    const nodeIds = new Set(fullGraph.nodes.map((node) => node.id));
    fullGraph.relationships = (snapshot.relationships || []).filter(
      (rel) => nodeIds.has(rel.source) && nodeIds.has(rel.target),
    );
    graph.nodes = fullGraph.nodes;
    graph.relationships = fullGraph.relationships;
    nodeCountEl.textContent = graph.nodes.length.toString();
    relationshipCountEl.textContent = graph.relationships.length.toString();

    render(topologyChanged);
    if (!selected && graph.nodes.length > 0) {
      selectItem({ kind: "node", value: graph.nodes[0] });
    } else if (selected?.kind === "node") {
      const node = graph.nodes.find((item) => item.id === selected.value.id);
      if (node) selected.value = node;
    } else if (selected?.kind === "relationship") {
      const relationship = graph.relationships.find((item) => item.id === selected.value.id);
      if (relationship) selected.value = relationship;
    }
  }

  function render(reheat = false) {
    const links = linkLayer
      .selectAll("line")
      .data(graph.relationships, (rel) => rel.id || `${rel.source}:${rel.type}:${rel.target}`);

    links.exit().remove();
    links
      .enter()
      .append("line")
      .attr("class", "link")
      .on("click", (event, rel) => {
        event.stopPropagation();
        selectItem({ kind: "relationship", value: rel });
      });

    const linkLabels = labelLayer
      .selectAll("text")
      .data(graph.relationships, (rel) => rel.id || `${rel.source}:${rel.type}:${rel.target}`);

    linkLabels.exit().remove();
    linkLabels
      .enter()
      .append("text")
      .attr("class", "link-label")
      .text((rel) => compactRelationship(rel.type));

    const nodes = nodeLayer.selectAll("g").data(graph.nodes, (node) => node.id);
    nodes.exit().remove();

    const entered = nodes
      .enter()
      .append("g")
      .attr("class", "node")
      .call(
        d3
          .drag()
          .on("start", dragStarted)
          .on("drag", dragged)
          .on("end", dragEnded),
      )
      .on("click", (event, node) => {
        event.stopPropagation();
        selectItem({ kind: "node", value: node });
      });

    entered.append("circle");
    entered.append("text").attr("class", "node-icon").attr("dy", "0.03em");
    entered.append("text").attr("class", "node-label").attr("dy", "2.6em");
    entered.append("title");

    const mergedNodes = entered.merge(nodes);
    mergedNodes
      .classed("selected", (node) => selected?.kind === "node" && selected.value.id === node.id)
      .select("circle")
      .attr("r", nodeRadius)
      .attr("fill", (node) => styleForNode(node).color);
    mergedNodes.select(".node-icon").text((node) => styleForNode(node).icon);
    mergedNodes.select(".node-label").text(nodeLabel);
    mergedNodes.select("title").text((node) => `${nodeKind(node)}\n${node.id}`);

    linkLayer
      .selectAll("line")
      .classed(
        "selected",
        (rel) => selected?.kind === "relationship" && selected.value.id === rel.id,
      );

    simulation.nodes(graph.nodes);
    simulation.force("link").links(graph.relationships.map((rel) => ({ ...rel })));
    if (reheat) {
      simulation.alpha(Math.max(simulation.alpha(), 0.72)).restart();
    }
  }

  function ticked() {
    linkLayer
      .selectAll("line")
      .attr("x1", (rel) => endpoint(rel.source).x)
      .attr("y1", (rel) => endpoint(rel.source).y)
      .attr("x2", (rel) => endpoint(rel.target).x)
      .attr("y2", (rel) => endpoint(rel.target).y);

    labelLayer
      .selectAll("text")
      .attr("x", (rel) => (endpoint(rel.source).x + endpoint(rel.target).x) / 2)
      .attr("y", (rel) => (endpoint(rel.source).y + endpoint(rel.target).y) / 2);

    nodeLayer.selectAll("g").attr("transform", (node) => `translate(${node.x},${node.y})`);
  }

  function endpoint(value) {
    if (value && typeof value === "object") return value;
    return nodeState.get(value) || graph.nodes.find((node) => node.id === value) || { x: 0, y: 0 };
  }

  function selectItem(item) {
    selected = item;
    inspectorEmpty.hidden = true;
    inspectorContent.hidden = false;
    inspectorMedia.hidden = true;
    clearMediaPreview();
    if (item.kind === "node") {
      const node = item.value;
      inspectorIcon.textContent = styleForNode(node).icon;
      inspectorIcon.style.color = styleForNode(node).color;
      inspectorTitle.textContent = nodeLabel(node);
      inspectorKind.textContent = `${nodeKind(node)} node`;
      renderProperties({ id: node.id, labels: node.labels, ...(node.properties || {}) });
      loadNodeDetails(node);
    } else {
      const rel = item.value;
      inspectorIcon.textContent = "→";
      inspectorIcon.style.color = "var(--warn)";
      inspectorTitle.textContent = rel.type;
      inspectorKind.textContent = `${relationshipEndpoint(rel.source)} → ${relationshipEndpoint(rel.target)}`;
      renderProperties({ id: rel.id, source: relationshipEndpoint(rel.source), target: relationshipEndpoint(rel.target), ...(rel.properties || {}) });
    }
    render();
  }

  async function loadNodeDetails(node) {
    const requestId = ++detailRequestId;
    try {
      const response = await fetch(`/graph/node/${encodeURIComponent(node.id)}`);
      if (!response.ok) throw new Error(`detail request failed: ${response.status}`);
      const details = await response.json();
      if (requestId !== detailRequestId || selected?.kind !== "node" || selected.value.id !== node.id) {
        return;
      }
      selected.value = {
        ...node,
        labels: details.labels || node.labels,
        properties: details.properties || node.properties || {},
        relationships: details.relationships || [],
      };
      renderMediaPreview(selected.value);
      renderProperties(propertiesForNodeDetails(selected.value));
    } catch (err) {
      if (requestId === detailRequestId && selected?.kind === "node" && selected.value.id === node.id) {
        renderProperties({ id: node.id, labels: node.labels, detail_error: err.message, ...(node.properties || {}) });
      }
    }
  }

  function renderMediaPreview(node) {
    clearMediaPreview();
    const props = node.properties || {};
    const media = mediaForNode(node);
    const mime = media.mime;
    const base64 = media.base64;
    const text = typeof props.text === "string" ? props.text : "";
    const transcript = typeof props.transcript === "string" ? props.transcript : "";

    let preview = null;
    if (base64 && mime.startsWith("image/")) {
      preview = document.createElement("img");
      preview.alt = nodeLabel(node);
      preview.src = dataUrl(mime, base64);
      if (nodeKind(node) === "Face") preview.className = "face-preview";
    } else if (base64 && mime.startsWith("audio/")) {
      preview = document.createElement("audio");
      preview.controls = true;
      preview.preload = "metadata";
      preview.src = audioSource(props, mime, base64);
    } else if (base64 && mime.startsWith("video/")) {
      preview = document.createElement("video");
      preview.controls = true;
      preview.preload = "metadata";
      preview.src = dataUrl(mime, base64);
    } else if (text || transcript) {
      preview = document.createElement("pre");
      preview.textContent = text || transcript;
    }

    if (!preview) {
      inspectorMedia.hidden = true;
      return;
    }
    inspectorMedia.hidden = false;
    inspectorMedia.append(preview);
  }

  function clearMediaPreview() {
    if (mediaObjectUrl) {
      URL.revokeObjectURL(mediaObjectUrl);
      mediaObjectUrl = "";
    }
    inspectorMedia.replaceChildren();
  }

  function propertiesForNodeDetails(node) {
    const props = { id: node.id, labels: node.labels, ...(node.properties || {}) };
    if (node.relationships?.length) {
      props.relationships = node.relationships.map((rel) => {
        const source = relationshipEndpoint(rel.source);
        const target = relationshipEndpoint(rel.target);
        return `${source === node.id ? "out" : "in"} ${rel.type} ${source === node.id ? target : source}`;
      });
    }
    return props;
  }

  function renderProperties(properties) {
    inspectorProperties.replaceChildren();
    Object.entries(properties)
      .filter(([, value]) => value !== null && value !== undefined && value !== "")
      .filter(([key]) => !largeMediaProperty(key))
      .filter(([key]) => !temporalProperty(key))
      .slice(0, 36)
      .forEach(([key, value]) => {
        const dt = document.createElement("dt");
        const dd = document.createElement("dd");
        dt.textContent = key;
        dd.textContent = formatValue(value);
        inspectorProperties.append(dt, dd);
      });
  }

  function nodeKind(node) {
    return (node.labels || []).find((label) => label !== "GraphNode") || "GraphNode";
  }

  function nodeLabel(node) {
    const props = node.properties || {};
    if (nodeKind(node) === "Face") {
      const index = Number.isFinite(Number(props.detection_index))
        ? ` #${Number(props.detection_index) + 1}`
        : "";
      return truncate(`face${index}`, 28);
    }
    const text =
      props.summary ||
      props.text ||
      props.object_label ||
      props.collection ||
      props.kind ||
      props.mime ||
      props.id ||
      node.id;
    return truncate(String(text), 28);
  }

  function styleForNode(node) {
    return palette[nodeKind(node)] || palette.default;
  }

  function nodeRadius(node) {
    if (nodeKind(node) === "Impression") return 24;
    if (nodeKind(node) === "Sensation") return 21;
    if (nodeKind(node) === "Face") return 22;
    return 19;
  }

  function mediaForNode(node) {
    const props = node.properties || {};
    if (nodeKind(node) === "Face") {
      return {
        mime: String(props.crop_mime || props.mime || "").toLowerCase(),
        base64:
          typeof props.crop_base64 === "string"
            ? props.crop_base64.trim()
            : typeof props.base64 === "string"
              ? props.base64.trim()
              : "",
      };
    }
    return {
      mime: String(props.mime || "").toLowerCase(),
      base64: typeof props.base64 === "string" ? props.base64.trim() : "",
    };
  }

  function largeMediaProperty(key) {
    return key === "base64" || key === "crop_base64";
  }

  function temporalProperty(key) {
    return /(^|_)(time|timestamp|date)(_|$)/i.test(key) || /_at$/i.test(key);
  }

  function compactRelationship(type) {
    return String(type || "").replace(/^HAS_/, "").replace(/_/g, " ").toLowerCase();
  }

  function relationshipEndpoint(value) {
    return value && typeof value === "object" ? value.id : value;
  }

  function truncate(value, length) {
    return value.length > length ? `${value.slice(0, length - 1)}…` : value;
  }

  function formatValue(value) {
    if (Array.isArray(value)) return value.join(", ");
    if (typeof value === "object") return JSON.stringify(value, null, 2);
    return String(value);
  }

  function dataUrl(mime, base64) {
    return `data:${mime};base64,${base64}`;
  }

  function audioSource(props, mime, base64) {
    if (mime.includes("format=s16") || mime.startsWith("audio/pcm") || mime.startsWith("audio/l16")) {
      const sampleRate = Number(props.sample_rate || 16000);
      const channels = Number(props.channels || 1);
      const wav = pcmS16ToWav(base64ToBytes(base64), sampleRate, channels);
      mediaObjectUrl = URL.createObjectURL(new Blob([wav], { type: "audio/wav" }));
      return mediaObjectUrl;
    }
    return dataUrl(mime, base64);
  }

  function base64ToBytes(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  }

  function pcmS16ToWav(pcmBytes, sampleRate, channels) {
    const header = new ArrayBuffer(44);
    const view = new DataView(header);
    writeAscii(view, 0, "RIFF");
    view.setUint32(4, 36 + pcmBytes.byteLength, true);
    writeAscii(view, 8, "WAVE");
    writeAscii(view, 12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);
    view.setUint16(22, channels, true);
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, sampleRate * channels * 2, true);
    view.setUint16(32, channels * 2, true);
    view.setUint16(34, 16, true);
    writeAscii(view, 36, "data");
    view.setUint32(40, pcmBytes.byteLength, true);

    const wav = new Uint8Array(44 + pcmBytes.byteLength);
    wav.set(new Uint8Array(header), 0);
    wav.set(pcmBytes, 44);
    return wav;
  }

  function writeAscii(view, offset, text) {
    for (let i = 0; i < text.length; i += 1) {
      view.setUint8(offset + i, text.charCodeAt(i));
    }
  }

  function signatureForSnapshot(snapshot) {
    const nodes = (snapshot.nodes || [])
      .map((node) => [
        node.id,
        [...(node.labels || [])].sort().join("|"),
        stableStringify(structuralProperties(node.properties || {})),
      ])
      .sort((a, b) => a[0].localeCompare(b[0]));
    const relationships = (snapshot.relationships || [])
      .map((rel) => [
        rel.id,
        relationshipEndpoint(rel.source),
        relationshipEndpoint(rel.target),
        rel.type,
        stableStringify(structuralProperties(rel.properties || {})),
      ])
      .sort((a, b) => a[0].localeCompare(b[0]));
    return stableStringify({ nodes, relationships });
  }

  function structuralProperties(properties) {
    return Object.fromEntries(
      Object.entries(properties).filter(([key]) => !temporalProperty(key)),
    );
  }

  function signatureForTopology(snapshot) {
    const nodes = (snapshot.nodes || []).map((node) => node.id).sort();
    const relationships = (snapshot.relationships || [])
      .map((rel) => `${relationshipEndpoint(rel.source)}:${rel.type}:${relationshipEndpoint(rel.target)}`)
      .sort();
    return stableStringify({ nodes, relationships });
  }

  function stableStringify(value) {
    if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
    if (!value || typeof value !== "object") return JSON.stringify(value);
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableStringify(value[key])}`)
      .join(",")}}`;
  }

  function dragStarted(event, node) {
    if (!event.active) simulation.alphaTarget(0.3).restart();
    node.fx = node.x;
    node.fy = node.y;
  }

  function dragged(event, node) {
    node.fx = event.x;
    node.fy = event.y;
  }

  function dragEnded(event, node) {
    if (!event.active) simulation.alphaTarget(0);
    node.fx = event.x;
    node.fy = event.y;
  }

  function resize() {
    const rect = svg.node().getBoundingClientRect();
    simulation.force("center", d3.forceCenter(rect.width / 2, rect.height / 2));
    simulation.alpha(0.3).restart();
  }

  function zoomBy(factor) {
    svg.transition().duration(160).call(zoom.scaleBy, factor);
  }

  function fitGraph() {
    const rect = svg.node().getBoundingClientRect();
    if (!graph.nodes.length || rect.width === 0 || rect.height === 0) return;
    const xs = graph.nodes.map((node) => node.x || rect.width / 2);
    const ys = graph.nodes.map((node) => node.y || rect.height / 2);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const width = Math.max(maxX - minX, 1);
    const height = Math.max(maxY - minY, 1);
    const scale = Math.min(2.5, 0.86 / Math.max(width / rect.width, height / rect.height));
    const tx = rect.width / 2 - scale * (minX + width / 2);
    const ty = rect.height / 2 - scale * (minY + height / 2);
    svg.transition().duration(220).call(zoom.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
  }

  svg.on("click", () => {
    selected = null;
    inspectorEmpty.hidden = false;
    inspectorContent.hidden = true;
    render();
  });

  document.getElementById("zoom-in").addEventListener("click", () => zoomBy(1.25));
  document.getElementById("zoom-out").addEventListener("click", () => zoomBy(0.8));
  document.getElementById("zoom-fit").addEventListener("click", fitGraph);
  window.addEventListener("resize", resize);

  resize();
  connect();
})();
