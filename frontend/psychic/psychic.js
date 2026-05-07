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
  const graphShell = document.querySelector(".graph-shell");
  const timelineShell = document.getElementById("timeline-shell");
  const statusEl = document.getElementById("status");
  const nodeCountEl = document.getElementById("node-count");
  const relationshipCountEl = document.getElementById("relationship-count");
  const graphModeEl = document.getElementById("graph-mode");
  const timelineModeEl = document.getElementById("timeline-mode");
  const allLabelFiltersEl = document.getElementById("all-label-filters");
  const allPredicateFiltersEl = document.getElementById("all-predicate-filters");
  const labelFiltersEl = document.getElementById("label-filters");
  const predicateFiltersEl = document.getElementById("predicate-filters");
  const timelineRowsEl = document.getElementById("timeline-rows");
  const timelineBoardEl = document.getElementById("timeline-board");
  const timelineRulerEl = document.getElementById("timeline-ruler");
  const timelinePlayheadEl = document.getElementById("timeline-playhead");
  const timelineSelectionEl = document.getElementById("timeline-selection");
  const timelineScrubEl = document.getElementById("timeline-scrub");
  const timelinePlayEl = document.getElementById("timeline-play");
  const timelinePauseEl = document.getElementById("timeline-pause");
  const timelineZoomInEl = document.getElementById("timeline-zoom-in");
  const timelineZoomOutEl = document.getElementById("timeline-zoom-out");
  const timelineZoomResetEl = document.getElementById("timeline-zoom-reset");
  const timelineRangeEl = document.getElementById("timeline-range");
  const presentPanelEl = document.getElementById("present-panel");
  const presentTimeEl = document.getElementById("present-time");
  const presentMediaEl = document.getElementById("present-media");
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
    Cluster: { color: "#f6e05e", icon: "C" },
    Theme: { color: "#fbbf24", icon: "T" },
    ClusterDiscoveryRun: { color: "#94a3b8", icon: "R" },
    ClusterThemeRun: { color: "#94a3b8", icon: "R" },
    Vector: { color: "#81ecec", icon: "V" },
    Geolocation: { color: "#a3e635", icon: "⌖" },
    Heartbeat: { color: "#ff7a90", icon: "♥" },
    ObjectObservation: { color: "#d2b48c", icon: "O" },
    Face: { color: "#ffcf99", icon: "◐" },
    FaceInstance: { color: "#ffcf99", icon: "◐" },
    VoiceSignature: { color: "#7dd3fc", icon: "V" },
    Voice: { color: "#7dd3fc", icon: "V" },
    Place: { color: "#a3e635", icon: "⌖" },
    Scene: { color: "#63d2ff", icon: "◉" },
    ImageCluster: { color: "#63d2ff", icon: "◉" },
    ImageTheme: { color: "#fbbf24", icon: "T" },
    MemoryCluster: { color: "#b8a1ff", icon: "M" },
    Person: { color: "#f0abfc", icon: "P" },
    RawPayload: { color: "#b8c0cc", icon: "R" },
    default: { color: "#c9d4de", icon: "◎" },
  };

  const fullGraph = { nodes: [], relationships: [] };
  const graph = { nodes: [], relationships: [] };
  const graphStore = {
    nodes: new Map(),
    relationships: new Map(),
  };
  const nodeState = new Map();
  const timelineDetailLoadingIds = new Set();
  const timelineImagePreloadCache = new Map();
  const filters = {
    labels: new Map(),
    predicates: new Map(),
  };
  const filterStorageKey = "psychic.graph.filters.v1";
  const graphCacheDbName = "psychic.graph.cache.v1";
  const graphCacheDbVersion = 1;
  const maxEmbeddingLinksPerCluster = 80;
  const temporalMarginRatio = 0.12;
  const timelineModeStorageKey = "psychic.view.mode.v1";
  const timelineDetailLoadLimit = 36;
  const timelineMinClipMs = 850;
  const timelineDefaultClipMs = 3000;
  const timelinePlaybackWindowMs = 12000;
  const timelineRows = [
    { id: "images", label: "Images", kinds: ["Image", "FaceInstance"] },
    { id: "audio", label: "Audio Clips", kinds: ["AudioClip"] },
    { id: "speech", label: "Speech Segments", kinds: ["SpeechSegment"] },
  ];
  const temporalLayoutPropertyKeys = [
    "occurred_at",
    "observed_at",
    "captured_at",
    "transcribed_at",
    "timestamp",
  ];
  let selected = null;
  let socket = null;
  let lastSnapshotSignature = "";
  let lastTopologySignature = "";
  let lastTemporalSignature = "";
  let lastFilterOptionsSignature = "";
  let detailRequestId = 0;
  let mediaObjectUrl = "";
  let viewMode = storedViewMode();
  let timelineFullExtent = null;
  let temporalExtent = null;
  let timelineExtent = null;
  let timelineCursor = null;
  let timelinePlaying = false;
  let timelinePlaybackFrame = 0;
  let timelinePlaybackLastTick = 0;
  let presentImageNodeId = "";
  let presentImageLoadingId = "";
  let timelineSelection = null;
  let pendingLocationTarget = targetFromLocation();
  let graphCacheDbPromise = null;
  let graphCacheSaveTimer = 0;

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
        .distance(linkDistance)
        .strength(linkStrength),
    )
    .force("charge", d3.forceManyBody().strength(chargeStrength))
    .force("center", d3.forceCenter())
    .force("theme-x", d3.forceX().strength(themeCenterStrength))
    .force("theme-y", d3.forceY().strength(themeCenterStrength))
    .force("time-x", d3.forceX(temporalX).strength(temporalXStrength))
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

    const previousTopologySignature = signatureForTopology(fullGraph);
    const previousTemporalSignature = signatureForTemporalLayout(fullGraph);
    const changed = mergeGraphSnapshot(snapshot, { persist: true });
    const topologySignature = signatureForTopology(fullGraph);
    const topologyChanged = topologySignature !== lastTopologySignature;
    lastTopologySignature = topologySignature;
    const temporalSignature = signatureForTemporalLayout(fullGraph);
    const temporalChanged = temporalSignature !== lastTemporalSignature;
    lastTemporalSignature = temporalSignature;

    if (!changed && previousTopologySignature === topologySignature && previousTemporalSignature === temporalSignature) {
      statusEl.textContent = "Live";
      return;
    }
    syncFilterControls();
    applyGraphFilters(topologyChanged || temporalChanged);
  }

  loadStoredFilters();
  setViewMode(viewMode, { persist: false });
  restoreGraphCache().catch(() => {
    statusEl.textContent = "Connecting";
  }).finally(connect);

  function mergeGraphSnapshot(snapshot, options = {}) {
    let changed = false;
    (snapshot.nodes || []).forEach((node) => {
      changed = mergeGraphNode(node, { persist: false }) || changed;
    });

    const nodeIds = new Set(graphStore.nodes.keys());
    (snapshot.relationships || []).forEach((rel) => {
      if (!nodeIds.has(relationshipEndpoint(rel.source)) || !nodeIds.has(relationshipEndpoint(rel.target))) {
        return;
      }
      changed = mergeGraphRelationship(rel, { persist: false }) || changed;
    });

    if (changed) {
      materializeFullGraph();
      if (options.persist !== false) scheduleGraphCacheSave();
    }
    return changed;
  }

  async function restoreGraphCache() {
    const db = await openGraphCacheDb();
    if (!db) return;
    const cached = await readGraphCache(db);
    if (!cached.nodes.length && !cached.relationships.length) return;

    mergeGraphSnapshot(cached, { persist: false });
    syncFilterControls();
    applyGraphFilters(true);
    statusEl.textContent = "Cached";
  }

  function openGraphCacheDb() {
    if (!("indexedDB" in window)) return Promise.resolve(null);
    if (graphCacheDbPromise) return graphCacheDbPromise;
    graphCacheDbPromise = new Promise((resolve) => {
      const request = window.indexedDB.open(graphCacheDbName, graphCacheDbVersion);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains("nodes")) {
          db.createObjectStore("nodes", { keyPath: "id" });
        }
        if (!db.objectStoreNames.contains("relationships")) {
          db.createObjectStore("relationships", { keyPath: "id" });
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => resolve(null);
      request.onblocked = () => resolve(null);
    });
    return graphCacheDbPromise;
  }

  function readGraphCache(db) {
    return new Promise((resolve) => {
      const transaction = db.transaction(["nodes", "relationships"], "readonly");
      const nodesRequest = transaction.objectStore("nodes").getAll();
      const relationshipsRequest = transaction.objectStore("relationships").getAll();
      transaction.oncomplete = () => {
        resolve({
          nodes: nodesRequest.result || [],
          relationships: relationshipsRequest.result || [],
        });
      };
      transaction.onerror = () => resolve({ nodes: [], relationships: [] });
      transaction.onabort = () => resolve({ nodes: [], relationships: [] });
    });
  }

  function scheduleGraphCacheSave() {
    if (graphCacheSaveTimer) window.clearTimeout(graphCacheSaveTimer);
    graphCacheSaveTimer = window.setTimeout(() => {
      graphCacheSaveTimer = 0;
      saveGraphCache().catch(() => {
        // Cache writes are best-effort; the live graph remains authoritative for this session.
      });
    }, 350);
  }

  async function saveGraphCache() {
    const db = await openGraphCacheDb();
    if (!db) return;
    await new Promise((resolve, reject) => {
      const transaction = db.transaction(["nodes", "relationships"], "readwrite");
      const nodeStore = transaction.objectStore("nodes");
      const relationshipStore = transaction.objectStore("relationships");
      graphStore.nodes.forEach((node) => nodeStore.put(serializeCachedNode(node)));
      graphStore.relationships.forEach((rel) => relationshipStore.put(serializeCachedRelationship(rel)));
      transaction.oncomplete = resolve;
      transaction.onerror = () => reject(transaction.error || new Error("graph cache write failed"));
      transaction.onabort = () => reject(transaction.error || new Error("graph cache write aborted"));
    });
  }

  function serializeCachedNode(node) {
    const cached = {
      id: node.id,
      labels: node.labels || [],
      properties: compactCachedProperties(node.properties || {}),
    };
    if (node.detailsCached) cached.detailsCached = true;
    if (Number.isFinite(node.x)) cached.x = node.x;
    if (Number.isFinite(node.y)) cached.y = node.y;
    if (Number.isFinite(node.fx)) cached.fx = node.fx;
    if (Number.isFinite(node.fy)) cached.fy = node.fy;
    if (Array.isArray(node.relationships)) {
      cached.relationships = node.relationships.map(serializeCachedRelationship);
    }
    return cached;
  }

  function serializeCachedRelationship(rel) {
    return {
      id: relationshipId(rel),
      source: relationshipEndpoint(rel.source),
      target: relationshipEndpoint(rel.target),
      type: rel.type,
      properties: compactCachedProperties(rel.properties || {}),
    };
  }

  function compactCachedProperties(properties) {
    return Object.fromEntries(
      Object.entries(properties).filter(([key]) => !largeMediaProperty(key)),
    );
  }

  function materializeFullGraph() {
    fullGraph.nodes = [...graphStore.nodes.values()];
    const nodeIds = new Set(graphStore.nodes.keys());
    fullGraph.relationships = [...graphStore.relationships.values()].filter(
      (rel) => nodeIds.has(relationshipEndpoint(rel.source)) && nodeIds.has(relationshipEndpoint(rel.target)),
    );
  }

  function syncFilterControls() {
    const labels = sortedUnique(
      fullGraph.nodes.flatMap((node) => node.labels || []),
    );
    const semanticSimilarity = semanticSimilarityRelationships(
      fullGraph.nodes,
      fullGraph.relationships,
      fullGraph.nodes,
    );
    const predicates = sortedUnique([
      ...fullGraph.relationships.map((rel) => rel.type).filter(Boolean),
      ...(embeddingNeighborRelationships(fullGraph.nodes, fullGraph.relationships).length
        ? ["SIMILAR_EMBEDDING"]
        : []),
      ...(semanticSimilarity.some((rel) => rel.type === "SIMILAR_FACE")
        ? ["SIMILAR_FACE"]
        : []),
      ...(semanticSimilarity.some((rel) => rel.type === "SIMILAR_VOICE_SIGNATURE")
        ? ["SIMILAR_VOICE_SIGNATURE"]
        : []),
    ]);

    labels.forEach((label) => ensureFilterOption(filters.labels, label));
    predicates.forEach((predicate) => ensureFilterOption(filters.predicates, predicate));

    const signature = stableStringify({ labels, predicates });
    if (signature !== lastFilterOptionsSignature) {
      lastFilterOptionsSignature = signature;
      renderFilterGroup(labelFiltersEl, "labels", labels);
      renderFilterGroup(predicateFiltersEl, "predicates", predicates);
    }
    syncFilterGroupControl("labels");
    syncFilterGroupControl("predicates");
  }

  function ensureFilterOption(group, value) {
    if (!group.has(value)) group.set(value, false);
  }

  function renderFilterGroup(container, kind, values) {
    if (!container) return;
    container.replaceChildren(
      ...values.map((value) => {
        const id = filterControlId(kind, value);
        const wrapper = document.createElement("label");
        const input = document.createElement("input");
        const text = document.createElement("span");
        wrapper.className = "filter-option";
        wrapper.htmlFor = id;
        input.id = id;
        input.type = "checkbox";
        input.checked = filterChecked(kind, value);
        input.addEventListener("change", () => {
          filterGroup(kind).set(value, input.checked);
          saveStoredFilters();
          syncFilterGroupControl(kind);
          applyGraphFilters(true);
        });
        text.textContent = value;
        text.title = value;
        wrapper.append(input, text);
        return wrapper;
      }),
    );
  }

  function applyGraphFilters(reheat = false) {
    graph.nodes = fullGraph.nodes.filter(nodeMatchesLabelFilters);
    const visibleNodeIds = new Set(graph.nodes.map((node) => node.id));
    const realRelationships = fullGraph.relationships.filter((rel) => {
      const source = relationshipEndpoint(rel.source);
      const target = relationshipEndpoint(rel.target);
      return visibleNodeIds.has(source) && visibleNodeIds.has(target) && predicateAllowed(rel.type);
    });
    const syntheticRelationships = [
      ...embeddingNeighborRelationships(graph.nodes, fullGraph.relationships),
      ...semanticSimilarityRelationships(graph.nodes, fullGraph.relationships, fullGraph.nodes),
    ].filter((rel) => predicateAllowed(rel.type));
    graph.relationships = [...realRelationships, ...syntheticRelationships];
    nodeCountEl.textContent = graph.nodes.length.toString();
    relationshipCountEl.textContent = graph.relationships.length.toString();

    syncSelectionWithFilteredGraph();
    render(reheat);
    renderTimeline();
    if (resolvePendingLocationTarget()) {
      return;
    }
    if (!selected && graph.nodes.length > 0) {
      selectItem({ kind: "node", value: graph.nodes[0] }, { updateUrl: false });
    }
  }

  function syncSelectionWithFilteredGraph() {
    if (selected?.kind === "node") {
      const node = graph.nodes.find((item) => item.id === selected.value.id);
      if (node) {
        selected.value = node;
        return;
      }
    } else if (selected?.kind === "relationship") {
      const relationship = graph.relationships.find((item) => relationshipId(item) === relationshipId(selected.value));
      if (relationship) {
        selected.value = relationship;
        return;
      }
    } else {
      return;
    }
    clearSelection({ updateUrl: false });
  }

  function render(reheat = false) {
    updateTemporalExtent();
    const links = linkLayer
      .selectAll("line")
      .data(graph.relationships, (rel) => rel.id || `${rel.source}:${rel.type}:${rel.target}`);

    links.exit().remove();
    const enteredLinks = links
      .enter()
      .append("line")
      .on("click", (event, rel) => {
        event.stopPropagation();
        selectItem({ kind: "relationship", value: rel, focusNodeId: relationshipEndpoint(rel.target) });
      });
    enteredLinks
      .merge(links)
      .attr("class", linkClass)
      .attr("stroke-width", linkStrokeWidth)
      .attr("opacity", linkOpacity);

    const linkLabels = labelLayer
      .selectAll("text")
      .data(graph.relationships, (rel) => rel.id || `${rel.source}:${rel.type}:${rel.target}`);

    linkLabels.exit().remove();
    linkLabels
      .enter()
      .append("text")
      .attr("class", "link-label")
      .merge(linkLabels)
      .text(linkLabel);

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
    entered.append("text").attr("class", "node-label");
    entered.append("title");

    const mergedNodes = entered.merge(nodes);
    mergedNodes
      .classed("selected", (node) => selected?.kind === "node" && selected.value.id === node.id)
      .select("circle")
      .attr("r", nodeRadius)
      .attr("fill", (node) => styleForNode(node).color);
    mergedNodes.select(".node-icon").text((node) => styleForNode(node).icon);
    mergedNodes
      .select(".node-label")
      .attr("dy", (node) => `${nodeRadius(node) + 10}px`)
      .text(nodeLabel);
    mergedNodes.select("title").text((node) => `${nodeKind(node)}\n${node.id}`);

    linkLayer
      .selectAll("line")
      .classed(
        "selected",
        (rel) => selected?.kind === "relationship" && relationshipId(selected.value) === relationshipId(rel),
      );

    simulation.nodes(graph.nodes);
    simulation.force("link").links(graph.relationships.map((rel) => ({ ...rel })));
    simulation.force("time-x").x(temporalX);
    if (reheat) {
      simulation.alpha(Math.max(simulation.alpha(), 0.72)).restart();
    }
  }

  function storedViewMode() {
    try {
      return window.localStorage.getItem(timelineModeStorageKey) === "timeline" ? "timeline" : "graph";
    } catch (_err) {
      return "graph";
    }
  }

  function setViewMode(mode, options = {}) {
    viewMode = mode === "timeline" ? "timeline" : "graph";
    graphShell.hidden = viewMode !== "graph";
    timelineShell.hidden = viewMode !== "timeline";
    graphModeEl.classList.toggle("active", viewMode === "graph");
    timelineModeEl.classList.toggle("active", viewMode === "timeline");
    graphModeEl.setAttribute("aria-pressed", viewMode === "graph" ? "true" : "false");
    timelineModeEl.setAttribute("aria-pressed", viewMode === "timeline" ? "true" : "false");
    presentPanelEl.hidden = viewMode !== "timeline";
    if (options.persist !== false) {
      try {
        window.localStorage.setItem(timelineModeStorageKey, viewMode);
      } catch (_err) {
        // View preference is optional.
      }
    }
    if (viewMode === "timeline") {
      renderTimeline();
    } else {
      pauseTimeline();
      resize();
    }
  }

  function renderTimeline() {
    if (!timelineRowsEl || !timelineRulerEl || !timelinePlayheadEl || !timelineScrubEl) return;
    const items = timelineItems();
    updateTimelineExtent(items);
    timelineRowsEl.replaceChildren();

    timelineRows.forEach((row) => {
      const rowEl = document.createElement("div");
      const label = document.createElement("div");
      const track = document.createElement("div");
      rowEl.className = "timeline-row";
      label.className = "timeline-row-label";
      label.textContent = row.label;
      track.className = "timeline-track";
      track.dataset.row = row.id;

      items
        .filter((item) => item.rowId === row.id)
        .filter(timelineItemVisible)
        .forEach((item) => track.append(timelineClipElement(item)));

      rowEl.append(label, track);
      timelineRowsEl.append(rowEl);
    });

    renderTimelineRuler();
    syncTimelineScrubber();
    renderPresentInstant();
    hydrateTimelineMedia(items);
  }

  function timelineItems() {
    return graph.nodes
      .map((node) => {
        const row = timelineRowForNode(node);
        const start = nodeTimestamp(node);
        if (!row || start === null) return null;
        const duration = timelineDurationMs(node);
        return {
          node,
          kind: nodeKind(node),
          rowId: row.id,
          start,
          end: start + duration,
          duration,
        };
      })
      .filter(Boolean)
      .sort((left, right) => left.start - right.start || left.node.id.localeCompare(right.node.id));
  }

  function timelineRowForNode(node) {
    const kind = nodeKind(node);
    return timelineRows.find((row) => row.kinds.includes(kind)) || null;
  }

  function updateTimelineExtent(items) {
    if (!items.length) {
      timelineFullExtent = null;
      timelineExtent = null;
      timelineCursor = null;
      pauseTimeline();
      timelineRangeEl.textContent = "No timed media";
      return;
    }
    const min = Math.min(...items.map((item) => item.start));
    const max = Math.max(...items.map((item) => item.end), min + timelineDefaultClipMs);
    timelineFullExtent = { min, max };
    if (!timelineExtent || timelineExtent.max <= min || timelineExtent.min >= max) {
      timelineExtent = { min, max };
    } else {
      timelineExtent = clampTimelineWindow(timelineExtent.min, timelineExtent.max);
    }
    if (timelineCursor === null || timelineCursor < timelineExtent.min || timelineCursor > timelineExtent.max) {
      timelineCursor = timelineExtent.min;
    }
    timelineRangeEl.textContent = `${formatTimelineInstant(timelineExtent.min)} to ${formatTimelineInstant(timelineExtent.max)}`;
  }

  function timelineDurationMs(node) {
    const props = node.properties || {};
    const duration = numericProperty(props, "duration_ms") ?? numericProperty(props, "clip_duration_ms");
    if (duration !== null && duration > 0) return Math.max(duration, timelineMinClipMs);
    const startMs = numericProperty(props, "start_ms");
    const endMs = numericProperty(props, "end_ms");
    if (startMs !== null && endMs !== null && endMs > startMs) {
      return Math.max(endMs - startMs, timelineMinClipMs);
    }
    const media = mediaForNode(node);
    if (media.base64 && media.mime && nodeKind(node) === "AudioClip") {
      const audioDuration = audioDurationFromProperties(props, media);
      if (audioDuration !== null) return Math.max(audioDuration, timelineMinClipMs);
    }
    return nodeKind(node) === "AudioClip" ? timelineDefaultClipMs : timelineMinClipMs;
  }

  function audioDurationFromProperties(props, media) {
    if (!media.mime.includes("format=s16") && !media.mime.startsWith("audio/pcm") && !media.mime.startsWith("audio/l16")) {
      return null;
    }
    const sampleRate = Number(props.sample_rate || 16000);
    const channels = Number(props.channels || 1);
    if (!Number.isFinite(sampleRate) || sampleRate <= 0 || !Number.isFinite(channels) || channels <= 0) {
      return null;
    }
    return (base64ToBytes(media.base64).byteLength / (sampleRate * channels * 2)) * 1000;
  }

  function timelineClipElement(item) {
    const clip = document.createElement("button");
    const media = mediaForNode(item.node);
    const visibleStart = Math.max(item.start, timelineExtent.min);
    const visibleEnd = Math.min(item.end, timelineExtent.max);
    const left = timelinePercent(visibleStart);
    const width = Math.max(timelinePercent(visibleEnd) - left, 0.15);
    clip.type = "button";
    clip.className = `timeline-clip timeline-clip-${item.rowId}`;
    clip.style.left = `${left}%`;
    clip.style.width = `${width}%`;
    clip.title = `${nodeKind(item.node)}: ${nodeLabel(item.node)}`;
    clip.classList.toggle("selected", selected?.kind === "node" && selected.value.id === item.node.id);
    clip.addEventListener("click", () => selectItem({ kind: "node", value: item.node }));

    if (media.base64 && media.mime.startsWith("image/")) {
      const image = document.createElement("img");
      image.alt = nodeLabel(item.node);
      image.src = dataUrl(media.mime, media.base64);
      clip.append(image);
    } else {
      const label = document.createElement("span");
      label.textContent = timelineClipLabel(item.node);
      clip.append(label);
    }
    return clip;
  }

  function timelineItemVisible(item) {
    return !!timelineExtent && item.start <= timelineExtent.max && item.end >= timelineExtent.min;
  }

  function timelineClipLabel(node) {
    if (nodeKind(node) === "AudioClip") return "audio";
    if (nodeKind(node) === "SpeechSegment") return "speech";
    return nodeLabel(node);
  }

  function renderTimelineRuler() {
    timelineRulerEl.replaceChildren();
    if (!timelineExtent) {
      updateTimelinePlayhead();
      return;
    }
    const labelSpacer = document.createElement("div");
    const track = document.createElement("div");
    labelSpacer.className = "timeline-ruler-spacer";
    track.className = "timeline-ruler-track";
    timelineTicks().forEach((tick) => {
      const mark = document.createElement("div");
      const line = document.createElement("span");
      const text = document.createElement("small");
      mark.className = "timeline-tick";
      mark.style.left = `${timelinePercent(tick)}%`;
      text.textContent = formatTimelineTick(tick);
      mark.append(line, text);
      track.append(mark);
    });
    timelineRulerEl.append(labelSpacer, track);
    updateTimelinePlayhead();
  }

  function timelineTicks() {
    if (!timelineExtent) return [];
    const span = Math.max(timelineExtent.max - timelineExtent.min, 1);
    const count = Math.min(7, Math.max(3, Math.floor(timelineRulerEl.getBoundingClientRect().width / 180)));
    return Array.from({ length: count }, (_item, index) =>
      timelineExtent.min + (span * index) / Math.max(count - 1, 1),
    );
  }

  function syncTimelineScrubber() {
    timelineScrubEl.disabled = !timelineExtent;
    timelineScrubEl.value = timelineExtent ? String(Math.round(timelinePercent(timelineCursor) * 10)) : "0";
    updateTimelinePlayhead();
    syncTimelineZoomControls();
    renderPresentInstant();
  }

  function updateTimelinePlayhead() {
    if (!timelineExtent || timelineCursor === null) {
      timelinePlayheadEl.hidden = true;
      return;
    }
    timelinePlayheadEl.hidden = false;
    const boardWidth = timelineBoardWidth();
    const labelWidth = timelineLabelWidth();
    const x = labelWidth + (timelinePercent(timelineCursor) / 100) * Math.max(0, boardWidth - labelWidth);
    timelinePlayheadEl.style.left = `${x}px`;
  }

  function timelinePercent(timestamp) {
    if (!timelineExtent || timelineExtent.max === timelineExtent.min) return 0;
    return clamp01((timestamp - timelineExtent.min) / (timelineExtent.max - timelineExtent.min)) * 100;
  }

  function timestampAtTimelineClientX(clientX) {
    if (!timelineExtent) return null;
    const rect = timelineBoardEl.getBoundingClientRect();
    const labelWidth = timelineLabelWidth();
    const trackWidth = Math.max(1, rect.width - labelWidth);
    const x = clamp01((clientX - rect.left - labelWidth) / trackWidth);
    return timelineExtent.min + x * (timelineExtent.max - timelineExtent.min);
  }

  function timelineTrackClientX(clientX) {
    const rect = timelineBoardEl.getBoundingClientRect();
    const labelWidth = timelineLabelWidth();
    return Math.max(0, Math.min(rect.width - labelWidth, clientX - rect.left - labelWidth));
  }

  function zoomTimelineBy(factor) {
    if (!timelineExtent || !timelineFullExtent) return;
    const currentSpan = timelineExtent.max - timelineExtent.min;
    const nextSpan = currentSpan * factor;
    zoomTimelineToSpan(nextSpan, timelineCursor ?? timelineExtent.min + currentSpan / 2);
  }

  function zoomTimelineToSpan(span, center) {
    if (!timelineFullExtent) return;
    const fullSpan = timelineFullExtent.max - timelineFullExtent.min;
    const clampedSpan = Math.max(timelineMinClipMs, Math.min(span, fullSpan));
    const half = clampedSpan / 2;
    setTimelineWindow(center - half, center + half);
  }

  function setTimelineWindow(min, max) {
    if (!timelineFullExtent || max <= min) return;
    timelineExtent = clampTimelineWindow(min, max);
    timelineCursor = Math.max(timelineExtent.min, Math.min(timelineCursor ?? timelineExtent.min, timelineExtent.max));
    renderTimeline();
  }

  function resetTimelineZoom() {
    if (!timelineFullExtent) return;
    timelineExtent = { ...timelineFullExtent };
    timelineCursor = Math.max(timelineExtent.min, Math.min(timelineCursor ?? timelineExtent.min, timelineExtent.max));
    renderTimeline();
  }

  function playTimeline() {
    if (!timelineExtent || timelinePlaying) return;
    if (timelineCursor === null || timelineCursor >= timelineExtent.max) {
      timelineCursor = timelineExtent.min;
      syncTimelineScrubber();
    }
    timelinePlaying = true;
    timelinePlaybackLastTick = performance.now();
    syncTimelinePlaybackControls();
    timelinePlaybackFrame = requestAnimationFrame(tickTimelinePlayback);
  }

  function pauseTimeline() {
    if (timelinePlaybackFrame) {
      cancelAnimationFrame(timelinePlaybackFrame);
      timelinePlaybackFrame = 0;
    }
    timelinePlaying = false;
    timelinePlaybackLastTick = 0;
    syncTimelinePlaybackControls();
  }

  function tickTimelinePlayback(now) {
    if (!timelinePlaying || !timelineExtent) {
      pauseTimeline();
      return;
    }
    const elapsed = Math.max(0, now - timelinePlaybackLastTick);
    timelinePlaybackLastTick = now;
    const span = timelineExtent.max - timelineExtent.min;
    timelineCursor = (timelineCursor ?? timelineExtent.min) + elapsed * (span / timelinePlaybackWindowMs);
    if (timelineCursor >= timelineExtent.max) {
      timelineCursor = timelineExtent.max;
      syncTimelineScrubber();
      pauseTimeline();
      return;
    }
    syncTimelineScrubber();
    timelinePlaybackFrame = requestAnimationFrame(tickTimelinePlayback);
  }

  function syncTimelinePlaybackControls() {
    const canPlay = !!timelineExtent;
    timelinePlayEl.disabled = !canPlay || timelinePlaying;
    timelinePauseEl.disabled = !timelinePlaying;
  }

  function clampTimelineWindow(min, max) {
    if (!timelineFullExtent) return { min, max };
    const fullMin = timelineFullExtent.min;
    const fullMax = timelineFullExtent.max;
    const fullSpan = fullMax - fullMin;
    const span = Math.max(timelineMinClipMs, Math.min(max - min, fullSpan));
    let nextMin = min;
    let nextMax = min + span;
    if (nextMin < fullMin) {
      nextMin = fullMin;
      nextMax = fullMin + span;
    }
    if (nextMax > fullMax) {
      nextMax = fullMax;
      nextMin = fullMax - span;
    }
    return { min: nextMin, max: nextMax };
  }

  function syncTimelineZoomControls() {
    const hasTimeline = !!timelineExtent && !!timelineFullExtent;
    const fullSpan = hasTimeline ? timelineFullExtent.max - timelineFullExtent.min : 0;
    const visibleSpan = hasTimeline ? timelineExtent.max - timelineExtent.min : 0;
    timelineZoomInEl.disabled = !hasTimeline || visibleSpan <= timelineMinClipMs;
    timelineZoomOutEl.disabled = !hasTimeline || visibleSpan >= fullSpan;
    timelineZoomResetEl.disabled = !hasTimeline || visibleSpan >= fullSpan;
    syncTimelinePlaybackControls();
  }

  function startTimelineSelection(event) {
    const target = event.target instanceof Element ? event.target : null;
    if (!timelineExtent || event.button !== 0 || target?.closest(".timeline-clip")) return;
    event.preventDefault();
    const x = timelineTrackClientX(event.clientX);
    timelineSelection = { pointerId: event.pointerId, startX: x, endX: x };
    timelineBoardEl.setPointerCapture(event.pointerId);
    updateTimelineSelectionOverlay();
  }

  function moveTimelineSelection(event) {
    if (!timelineSelection || timelineSelection.pointerId !== event.pointerId) return;
    timelineSelection.endX = timelineTrackClientX(event.clientX);
    updateTimelineSelectionOverlay();
  }

  function finishTimelineSelection(event) {
    if (!timelineSelection || timelineSelection.pointerId !== event.pointerId) return;
    const startX = timelineSelection.startX;
    const endX = timelineSelection.endX;
    clearTimelineSelectionOverlay();
    if (timelineBoardEl.hasPointerCapture(event.pointerId)) {
      timelineBoardEl.releasePointerCapture(event.pointerId);
    }

    const delta = Math.abs(endX - startX);
    const rect = timelineBoardEl.getBoundingClientRect();
    const labelWidth = timelineLabelWidth();
    if (delta < 12) {
      timelineCursor = timestampAtTimelineClientX(event.clientX);
      syncTimelineScrubber();
      return;
    }

    const trackWidth = Math.max(1, rect.width - labelWidth);
    const leftRatio = Math.min(startX, endX) / trackWidth;
    const rightRatio = Math.max(startX, endX) / trackWidth;
    const span = timelineExtent.max - timelineExtent.min;
    setTimelineWindow(
      timelineExtent.min + leftRatio * span,
      timelineExtent.min + rightRatio * span,
    );
  }

  function cancelTimelineSelection(event) {
    if (!timelineSelection || timelineSelection.pointerId !== event.pointerId) return;
    clearTimelineSelectionOverlay();
  }

  function updateTimelineSelectionOverlay() {
    if (!timelineSelection) return;
    const labelWidth = timelineLabelWidth();
    const left = Math.min(timelineSelection.startX, timelineSelection.endX);
    const width = Math.abs(timelineSelection.endX - timelineSelection.startX);
    timelineSelectionEl.hidden = false;
    timelineSelectionEl.style.left = `${labelWidth + left}px`;
    timelineSelectionEl.style.width = `${width}px`;
  }

  function clearTimelineSelectionOverlay() {
    timelineSelection = null;
    timelineSelectionEl.hidden = true;
    timelineSelectionEl.style.width = "0";
  }

  function timelineBoardWidth() {
    return document.getElementById("timeline-board")?.getBoundingClientRect().width || 0;
  }

  function timelineLabelWidth() {
    return timelineRowsEl.querySelector(".timeline-row-label")?.getBoundingClientRect().width || 132;
  }

  function formatTimelineInstant(timestamp) {
    const date = new Date(timestamp);
    if (!Number.isFinite(date.getTime())) return "";
    return date.toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  function formatTimelineTick(timestamp) {
    const date = new Date(timestamp);
    if (!Number.isFinite(date.getTime())) return "";
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  }

  function renderPresentInstant() {
    if (viewMode !== "timeline" || !presentPanelEl) return;
    if (timelineCursor === null) {
      presentTimeEl.textContent = "No instant";
      renderPresentPlaceholder("No image at head");
      return;
    }
    presentTimeEl.textContent = formatTimelineInstant(timelineCursor);
    const imageNode = latestImageAtTimelineCursor();
    if (!imageNode) {
      presentImageNodeId = "";
      presentImageLoadingId = "";
      renderPresentPlaceholder("No image at head");
      return;
    }
    const media = mediaForNode(imageNode);
    if (media.base64 && media.mime.startsWith("image/")) {
      presentImageNodeId = imageNode.id;
      const overlay = renderPresentImageFrame(imageNode, media);
      renderPresentSpeechOverlays(overlay);
      return;
    }
    presentImageNodeId = imageNode.id;
    renderPresentPlaceholder("Loading image");
    if (presentImageLoadingId === imageNode.id) return;
    presentImageLoadingId = imageNode.id;
    fetchNodeDetails(imageNode.id, { requireComplete: true })
      .then((details) => {
        if (presentImageLoadingId === imageNode.id) presentImageLoadingId = "";
        if (viewMode !== "timeline" || presentImageNodeId !== imageNode.id) return;
        mergeGraphNode(details);
        renderPresentInstant();
        renderTimeline();
      })
      .catch(() => {
        if (presentImageLoadingId === imageNode.id) presentImageLoadingId = "";
        if (presentImageNodeId === imageNode.id) renderPresentPlaceholder("Image unavailable");
      });
  }

  function renderPresentPlaceholder(text) {
    const placeholder = document.createElement("span");
    placeholder.textContent = text;
    presentMediaEl.replaceChildren(placeholder);
  }

  function renderPresentImageFrame(imageNode, media) {
    let frame = presentMediaEl.querySelector(".present-frame");
    let image = frame?.querySelector("img");
    let overlay = frame?.querySelector(".present-speech-overlay");
    const src = timelineImageSrcForNode(imageNode) || dataUrl(media.mime, media.base64);

    if (!frame || frame.dataset.nodeId !== imageNode.id) {
      frame = document.createElement("div");
      image = document.createElement("img");
      overlay = document.createElement("div");
      frame.className = "present-frame";
      frame.dataset.nodeId = imageNode.id;
      overlay.className = "present-speech-overlay";
      image.alt = nodeLabel(imageNode);
      image.src = src;
      frame.append(image, overlay);
      presentMediaEl.replaceChildren(frame);
    } else if (image.src !== src) {
      image.src = src;
    }

    return overlay;
  }

  function renderPresentSpeechOverlays(container) {
    container.replaceChildren(
      ...visibleSpeechSegments().map((segment, index) => {
        const el = document.createElement("div");
        const text = document.createElement("span");
        const left = timelinePercent(Math.max(segment.start, timelineExtent.min));
        const right = timelinePercent(Math.min(segment.end, timelineExtent.max));
        el.className = "present-speech-segment";
        el.classList.toggle("active", segment.start <= timelineCursor && segment.end >= timelineCursor);
        el.style.left = `${left}%`;
        el.style.width = `${Math.max(right - left, 4)}%`;
        el.style.bottom = `${0.55 + (index % 3) * 1.85}rem`;
        text.textContent = speechSegmentText(segment.node);
        el.append(text);
        return el;
      }),
    );
  }

  function visibleSpeechSegments() {
    if (!timelineExtent || timelineCursor === null) return [];
    return graph.nodes
      .filter((node) => nodeKind(node) === "SpeechSegment")
      .map((node) => {
        const start = nodeTimestamp(node);
        if (start === null) return null;
        return { node, start, end: start + timelineDurationMs(node) };
      })
      .filter(Boolean)
      .filter((segment) => segment.start <= timelineExtent.max && segment.end >= timelineExtent.min)
      .sort((left, right) => left.start - right.start || left.node.id.localeCompare(right.node.id));
  }

  function speechSegmentText(node) {
    const props = node.properties || {};
    return String(props.text || props.transcript || props.summary || nodeLabel(node));
  }

  function latestImageAtTimelineCursor() {
    if (timelineCursor === null) return null;
    return graph.nodes
      .filter((node) => nodeKind(node) === "Image")
      .map((node) => ({ node, timestamp: nodeTimestamp(node) }))
      .filter((item) => item.timestamp !== null && item.timestamp <= timelineCursor)
      .sort((left, right) => right.timestamp - left.timestamp || right.node.id.localeCompare(left.node.id))[0]?.node || null;
  }

  function hydrateTimelineMedia(items) {
    const visibleImageItems = items
      .filter((item) => (item.kind === "Image" || item.kind === "FaceInstance"))
      .filter(timelineItemVisible);
    pruneTimelineImagePreloadCache(new Set(visibleImageItems.map((item) => item.node.id)));
    visibleImageItems.forEach((item) => preloadTimelineImageNode(item.node));

    const loadable = visibleImageItems
      .filter((item) => !mediaForNode(item.node).base64)
      .filter((item) => !timelineDetailLoadingIds.has(item.node.id))
      .slice(0, timelineDetailLoadLimit);
    if (!loadable.length) return;

    Promise.all(loadable.map(async (item) => {
      timelineDetailLoadingIds.add(item.node.id);
      try {
        const details = await fetchNodeDetails(item.node.id, { requireComplete: true });
        preloadTimelineImageNode(details);
        return true;
      } catch (_err) {
        return false;
      } finally {
        timelineDetailLoadingIds.delete(item.node.id);
      }
    })).then((loaded) => {
      if (!loaded.some(Boolean)) return;
      materializeFullGraph();
      renderTimeline();
    });
  }

  function preloadTimelineImageNode(node) {
    const src = timelineImageDataSrcForNode(node);
    if (!src) return;
    const existing = timelineImagePreloadCache.get(node.id);
    if (existing?.src === src) return;
    const image = new Image();
    image.decoding = "async";
    image.src = src;
    timelineImagePreloadCache.set(node.id, { src, image });
    if (typeof image.decode === "function") {
      image.decode().catch(() => {
        // Browser image cache warming is opportunistic; display still uses the data URL.
      });
    }
  }

  function timelineImageSrcForNode(node) {
    const cached = timelineImagePreloadCache.get(node.id);
    if (cached) return cached.src;
    return timelineImageDataSrcForNode(node);
  }

  function timelineImageDataSrcForNode(node) {
    const media = mediaForNode(node);
    if (!media.base64 || !media.mime.startsWith("image/")) return "";
    return dataUrl(media.mime, media.base64);
  }

  function pruneTimelineImagePreloadCache(visibleIds) {
    timelineImagePreloadCache.forEach((_entry, nodeId) => {
      if (!visibleIds.has(nodeId) && nodeId !== presentImageNodeId) {
        timelineImagePreloadCache.delete(nodeId);
      }
    });
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

  function nodeMatchesLabelFilters(node) {
    return (node.labels || []).every((label) => filterChecked("labels", label));
  }

  function predicateAllowed(type) {
    return filterChecked("predicates", type);
  }

  function filterChecked(kind, value) {
    return filterGroup(kind).get(value) !== false;
  }

  function filterGroup(kind) {
    return kind === "labels" ? filters.labels : filters.predicates;
  }

  function syncFilterGroupControl(kind) {
    const control = allFilterControl(kind);
    if (!control) return;
    const values = [...filterGroup(kind).values()];
    const checkedCount = values.filter(Boolean).length;
    control.checked = values.length > 0 && checkedCount === values.length;
    control.indeterminate = checkedCount > 0 && checkedCount < values.length;
  }

  function setFilterGroup(kind, checked) {
    const group = filterGroup(kind);
    group.forEach((_value, key) => group.set(key, checked));
    saveStoredFilters();
    renderFilterGroup(filterContainer(kind), kind, sortedUnique([...group.keys()]));
    syncFilterGroupControl(kind);
    applyGraphFilters(true);
  }

  function allFilterControl(kind) {
    return kind === "labels" ? allLabelFiltersEl : allPredicateFiltersEl;
  }

  function filterContainer(kind) {
    return kind === "labels" ? labelFiltersEl : predicateFiltersEl;
  }

  function loadStoredFilters() {
    let stored;
    try {
      stored = JSON.parse(window.localStorage.getItem(filterStorageKey) || "{}");
    } catch (_err) {
      return;
    }
    restoreFilterGroup(filters.labels, stored.labels);
    restoreFilterGroup(filters.predicates, stored.predicates);
  }

  function restoreFilterGroup(group, values) {
    if (!values || typeof values !== "object" || Array.isArray(values)) return;
    Object.entries(values).forEach(([value, checked]) => {
      group.set(value, checked !== false);
    });
  }

  function saveStoredFilters() {
    try {
      window.localStorage.setItem(
        filterStorageKey,
        JSON.stringify({
          labels: Object.fromEntries(filters.labels),
          predicates: Object.fromEntries(filters.predicates),
        }),
      );
    } catch (_err) {
      // Ignore storage failures so filtering still works for this session.
    }
  }

  function sortedUnique(values) {
    return [...new Set(values)].sort((left, right) => left.localeCompare(right));
  }

  function filterControlId(kind, value) {
    const encoded = Array.from(String(value || "")).map((char) =>
      char.charCodeAt(0).toString(16).padStart(2, "0"),
    ).join("");
    return `filter-${kind}-${encoded || "empty"}`;
  }

  function embeddingNeighborRelationships(nodes, relationships) {
    return similarityRelationshipsForVectorClusters(nodes, relationships, {
      type: "SIMILAR_EMBEDDING",
      idPrefix: "synthetic:embedding-neighbor",
      targetsForVector: (vectorId) => [vectorId],
    });
  }

  function semanticSimilarityRelationships(nodes, relationships, contextNodes = nodes) {
    const visibleNodeIds = new Set(nodes.map((node) => node.id));
    const ownersByVector = vectorOwnersByKind(contextNodes, relationships);
    return [
      ...similarityRelationshipsForVectorClusters(contextNodes, relationships, {
        type: "SIMILAR_FACE",
        idPrefix: "synthetic:face-similarity",
        targetsForVector: (vectorId) => (ownersByVector.face.get(vectorId) || [])
          .filter((ownerId) => visibleNodeIds.has(ownerId)),
      }),
      ...similarityRelationshipsForVectorClusters(contextNodes, relationships, {
        type: "SIMILAR_VOICE_SIGNATURE",
        idPrefix: "synthetic:voice-signature-similarity",
        targetsForVector: (vectorId) => (ownersByVector.voiceSignature.get(vectorId) || [])
          .filter((ownerId) => visibleNodeIds.has(ownerId)),
      }),
    ];
  }

  function similarityRelationshipsForVectorClusters(nodes, relationships, options) {
    const nodesById = new Map(nodes.map((node) => [node.id, node]));
    const clusters = new Map();

    relationships.forEach((rel) => {
      if (rel.type !== "HAS_CLUSTER_MEMBER" && rel.type !== "MEMBER_OF_CLUSTER") return;
      const source = relationshipEndpoint(rel.source);
      const target = relationshipEndpoint(rel.target);
      const sourceNode = nodesById.get(source);
      const targetNode = nodesById.get(target);
      const clusterId = isClusterNode(sourceNode) ? source : isClusterNode(targetNode) ? target : "";
      const vectorId = isEmbeddingNode(sourceNode) ? source : isEmbeddingNode(targetNode) ? target : "";
      if (!clusterId || !vectorId) return;

      const clusterNode = nodesById.get(clusterId);
      const cluster = clusters.get(clusterId) || {
        id: clusterId,
        strength: numericProperty(clusterNode?.properties, "mean_similarity") ?? 0.65,
        members: new Map(),
      };
      const memberStrength = numericProperty(rel.properties, "average_similarity") ?? cluster.strength;
      const existing = cluster.members.get(vectorId);
      if (!existing || memberStrength > existing.strength) {
        cluster.members.set(vectorId, { id: vectorId, strength: memberStrength });
      }
      clusters.set(clusterId, cluster);
    });

    const byPair = new Map();
    clusters.forEach((cluster) => {
      const members = [...cluster.members.values()]
        .flatMap((member) => {
          const targets = options.targetsForVector(member.id)
            .filter((targetId) => nodesById.has(targetId));
          return targets.map((targetId) => ({
            id: targetId,
            vectorId: member.id,
            strength: member.strength,
          }));
        });
      const pairs = [];
      for (let left = 0; left < members.length; left += 1) {
        for (let right = left + 1; right < members.length; right += 1) {
          const source = members[left];
          const target = members[right];
          if (source.id === target.id) continue;
          const strength = clamp01((source.strength + target.strength + cluster.strength) / 3);
          pairs.push({
            source: source.id,
            target: target.id,
            strength,
            clusterId: cluster.id,
            sourceVectorId: source.vectorId,
            targetVectorId: target.vectorId,
          });
        }
      }
      pairs
        .sort((left, right) => right.strength - left.strength)
        .slice(0, maxEmbeddingLinksPerCluster)
        .forEach((pair) => {
          const key = [pair.source, pair.target].sort().join("|");
          const existing = byPair.get(key);
          if (!existing || pair.strength > existing.strength) {
            byPair.set(key, pair);
          }
        });
    });

    return [...byPair.values()]
      .sort((left, right) => left.source.localeCompare(right.source) || left.target.localeCompare(right.target))
      .map((pair) => ({
        id: `${options.idPrefix}:${pair.source}:${pair.target}`,
        source: pair.source,
        target: pair.target,
        type: options.type,
        synthetic: true,
        properties: {
          display_only: true,
          inferred_from_cluster: pair.clusterId,
          source_vector_id: pair.sourceVectorId,
          target_vector_id: pair.targetVectorId,
          strength: Number(pair.strength.toFixed(3)),
        },
      }));
  }

  function vectorOwnersByKind(nodes, relationships) {
    const nodesById = new Map(nodes.map((node) => [node.id, node]));
    const owners = {
      face: new Map(),
      voiceSignature: new Map(),
    };

    relationships.forEach((rel) => {
      if (rel.type !== "HAS_FACE_VECTOR" && rel.type !== "HAS_VOICE_VECTOR") return;
      const source = relationshipEndpoint(rel.source);
      const target = relationshipEndpoint(rel.target);
      const sourceNode = nodesById.get(source);
      const targetNode = nodesById.get(target);
      const vectorId = isEmbeddingNode(sourceNode) ? source : isEmbeddingNode(targetNode) ? target : "";
      if (!vectorId) return;

      const ownerId = source === vectorId ? target : source;
      const ownerNode = nodesById.get(ownerId);
      if (rel.type === "HAS_FACE_VECTOR" && nodeKind(ownerNode) === "FaceInstance") {
        addVectorOwner(owners.face, vectorId, ownerId);
      } else if (rel.type === "HAS_VOICE_VECTOR" && nodeKind(ownerNode) === "VoiceSignature") {
        addVectorOwner(owners.voiceSignature, vectorId, ownerId);
      }
    });

    return owners;
  }

  function addVectorOwner(owners, vectorId, ownerId) {
    const ownerIds = owners.get(vectorId) || [];
    if (!ownerIds.includes(ownerId)) ownerIds.push(ownerId);
    owners.set(vectorId, ownerIds);
  }

  function selectItem(item, options = {}) {
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
    if (options.updateUrl !== false) {
      updateUrlForSelection(item, options);
    }
    render();
    renderTimeline();
  }

  function clearSelection(options = {}) {
    selected = null;
    inspectorEmpty.hidden = false;
    inspectorContent.hidden = true;
    inspectorMedia.hidden = true;
    clearMediaPreview();
    if (options.updateUrl !== false) {
      clearGraphTargetUrl(options);
    }
    render();
    renderTimeline();
  }

  async function loadNodeDetails(node) {
    const requestId = ++detailRequestId;
    try {
      const details = await fetchNodeDetails(node.id, { requireComplete: true });
      if (requestId !== detailRequestId || selected?.kind !== "node" || selected.value.id !== node.id) {
        return;
      }
      selected.value = {
        ...node,
        labels: details.labels || node.labels,
        properties: details.properties || node.properties || {},
        relationships: details.relationships || [],
      };
      mergeGraphNode(selected.value);
      (selected.value.relationships || []).forEach((rel) => mergeGraphRelationship(rel));
      materializeFullGraph();
      scheduleGraphCacheSave();
      renderMediaPreview(selected.value);
      renderProperties(propertiesForNodeDetails(selected.value));
      renderTimeline();
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
    if (nodeKind(node) === "SpeechSegment") {
      preview = document.createElement("audio");
      preview.controls = true;
      preview.preload = "metadata";
      preview.src = speechSegmentAudioSrc(node);
    } else if (base64 && mime.startsWith("image/")) {
      preview = document.createElement("img");
      preview.alt = nodeLabel(node);
      preview.src = dataUrl(mime, base64);
      if (nodeKind(node) === "FaceInstance") preview.className = "face-preview";
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
      props.relationships = node.relationships.map((rel) => relationshipReferenceForNode(node.id, rel));
    }
    return props;
  }

  function renderProperties(properties) {
    inspectorProperties.replaceChildren();
    const entries = Object.entries(properties)
      .filter(([, value]) => value !== null && value !== undefined && value !== "")
      .filter(([key]) => !largeMediaProperty(key))
      .filter(([key]) => !temporalProperty(key));
    const relationshipEntries = entries.filter(([key]) => key === "relationships");
    const visibleEntries = [
      ...entries.filter(([key]) => key !== "relationships").slice(0, 36),
      ...relationshipEntries,
    ];
    visibleEntries
      .forEach(([key, value]) => {
        const dt = document.createElement("dt");
        const dd = document.createElement("dd");
        dt.textContent = key;
        if (key === "relationships" && Array.isArray(value)) {
          renderRelationshipLinks(dd, value);
        } else {
          dd.textContent = formatValue(value);
        }
        inspectorProperties.append(dt, dd);
      });
  }

  function renderRelationshipLinks(container, relationships) {
    const list = document.createElement("ul");
    list.className = "relationship-list";
    relationships.forEach((relationship) => {
      const item = document.createElement("li");
      const link = document.createElement("a");
      const target = {
        nodeId: relationship.otherId,
        relationshipId: relationshipId(relationship),
        relationship,
      };
      link.href = graphTargetHref(target);
      link.textContent = relationship.label || formatRelationshipReference(relationship);
      link.addEventListener("click", (event) => {
        event.preventDefault();
        navigateToGraphTarget(target, { updateUrl: true }).catch((err) => {
          statusEl.textContent = err.message || "Graph target unavailable";
        });
      });
      item.append(link);
      list.append(item);
    });
    container.append(list);
  }

  function relationshipReferenceForNode(nodeId, rel) {
    const source = relationshipEndpoint(rel.source);
    const target = relationshipEndpoint(rel.target);
    const outgoing = source === nodeId;
    const otherId = outgoing ? target : source;
    return {
      ...rel,
      source,
      target,
      otherId,
      direction: outgoing ? "out" : "in",
      label: `${outgoing ? "out" : "in"} ${rel.type} ${otherId}`,
    };
  }

  function formatRelationshipReference(relationship) {
    return `${relationship.direction || "rel"} ${relationship.type} ${relationship.otherId || relationshipEndpoint(relationship.target)}`;
  }

  function nodeKind(node) {
    const labels = node?.labels || [];
    return labels.find((label) => label !== "GraphNode" && label !== "Cluster")
      || labels.find((label) => label !== "GraphNode")
      || "GraphNode";
  }

  function nodeLabel(node) {
    const props = node.properties || {};
    if (nodeKind(node) === "FaceInstance") {
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
    if (nodeKind(node) === "Theme") return 35;
    if (hasNodeLabel(node, "Cluster")) return 31;
    if (nodeKind(node) === "Impression") return 24;
    if (nodeKind(node) === "Sensation") return 21;
    if (nodeKind(node) === "FaceInstance") return 22;
    if (nodeKind(node) === "VoiceSignature") return 22;
    return 19;
  }

  function linkStrength(link) {
    if (link.synthetic) return 0.25 + similarityStrength(link) * 1.15;
    return isThemeEndpoint(link) ? 0.95 : 0.45;
  }

  function chargeStrength(node) {
    return nodeKind(node) === "Theme" ? -210 : -520;
  }

  function themeCenterStrength(node) {
    return nodeKind(node) === "Theme" ? 0.18 : 0.015;
  }

  function temporalXStrength(node) {
    return nodeTimestamp(node) === null ? 0.01 : 0.12;
  }

  function updateTemporalExtent() {
    const timestamps = graph.nodes
      .map(nodeTimestamp)
      .filter((value) => value !== null)
      .sort((left, right) => left - right);
    temporalExtent = timestamps.length > 1
      ? { min: timestamps[0], max: timestamps[timestamps.length - 1] }
      : null;
  }

  function temporalX(node) {
    const rect = svg.node().getBoundingClientRect();
    const center = rect.width / 2;
    const timestamp = nodeTimestamp(node);
    if (timestamp === null || !temporalExtent || temporalExtent.max === temporalExtent.min) {
      return center;
    }
    const margin = Math.max(48, rect.width * temporalMarginRatio);
    const left = margin;
    const right = Math.max(left, rect.width - margin);
    const ratio = (timestamp - temporalExtent.min) / (temporalExtent.max - temporalExtent.min);
    return left + clamp01(ratio) * (right - left);
  }

  function nodeTimestamp(node) {
    const props = node.properties || {};
    for (const key of temporalLayoutKeys(node)) {
      const value = props[key];
      if (value === null || value === undefined || value === "") continue;
      if (typeof value === "number" && Number.isFinite(value)) return value;
      const timestamp = Date.parse(value);
      if (Number.isFinite(timestamp)) return timestamp;
    }
    return null;
  }

  function temporalLayoutKeys(node) {
    if (nodeKind(node) === "Sensation") return ["observed_at", "occurred_at", "captured_at", "timestamp"];
    if (nodeKind(node) === "AudioClip") return ["captured_at", "occurred_at", "observed_at", "timestamp"];
    if (nodeKind(node) === "Image") return ["captured_at", "occurred_at", "observed_at", "timestamp"];
    if (nodeKind(node) === "Transcription") return ["transcribed_at", "occurred_at", "captured_at", "timestamp"];
    return temporalLayoutPropertyKeys;
  }

  function isThemeEndpoint(link) {
    return nodeKind(endpoint(link.source)) === "Theme" || nodeKind(endpoint(link.target)) === "Theme";
  }

  function isEmbeddingNode(node) {
    if (!node) return false;
    const props = node.properties || {};
    return nodeKind(node) === "Vector" || node.id?.startsWith("qdrant:") || props.database === "qdrant";
  }

  function isClusterNode(node) {
    return !!node && (hasNodeLabel(node, "Cluster") || node.id?.startsWith("cluster:"));
  }

  function hasNodeLabel(node, label) {
    return (node?.labels || []).includes(label);
  }

  function similarityStrength(link) {
    return clamp01(numericProperty(link.properties, "strength") ?? 0.65);
  }

  function numericProperty(properties, key) {
    const value = properties?.[key];
    if (value === null || value === undefined || value === "") return null;
    const number = Number(value);
    return Number.isFinite(number) ? number : null;
  }

  function clamp01(value) {
    return Math.max(0, Math.min(1, value));
  }

  function formatStrength(value) {
    return value.toFixed(2);
  }

  function mediaForNode(node) {
    const props = node.properties || {};
    if (nodeKind(node) === "FaceInstance") {
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

  function linkLabel(rel) {
    if (rel.synthetic && rel.type.startsWith("SIMILAR_")) {
      return formatStrength(similarityStrength(rel));
    }
    return compactRelationship(rel.type);
  }

  function linkClass(rel) {
    if (!rel.synthetic) return "link";
    if (rel.type === "SIMILAR_FACE") return "link semantic-similarity-link face-similarity-link";
    if (rel.type === "SIMILAR_VOICE_SIGNATURE") return "link semantic-similarity-link voice-signature-similarity-link";
    return "link embedding-link";
  }

  function linkDistance(link) {
    if (link.synthetic) {
      return 32 + (1 - similarityStrength(link)) * 118;
    }
    return 90 + Math.min(link.type.length * 2, 70);
  }

  function linkStrokeWidth(link) {
    if (!link.synthetic) return null;
    return 1.1 + similarityStrength(link) * 3.6;
  }

  function linkOpacity(link) {
    if (!link.synthetic) return null;
    return 0.24 + similarityStrength(link) * 0.62;
  }

  function relationshipEndpoint(value) {
    return value && typeof value === "object" ? value.id : value;
  }

  function relationshipId(rel) {
    return rel?.id || `${relationshipEndpoint(rel?.source)}:${rel?.type}:${relationshipEndpoint(rel?.target)}`;
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

  function speechSegmentAudioSrc(node) {
    return `/graph/speech-segment/${encodeURIComponent(node.id)}/audio.wav`;
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
      Object.entries(properties).filter(([key]) => !temporalProperty(key) || temporalLayoutKey(key)),
    );
  }

  function temporalLayoutKey(key) {
    return temporalLayoutPropertyKeys.includes(key);
  }

  function signatureForTemporalLayout(snapshot) {
    const nodes = (snapshot.nodes || [])
      .map((node) => [node.id, nodeTimestamp(node)])
      .filter(([, timestamp]) => timestamp !== null)
      .sort((left, right) => left[0].localeCompare(right[0]));
    return stableStringify(nodes);
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

  function targetFromLocation() {
    const params = new URLSearchParams(window.location.search);
    const nodeId = params.get("node") || "";
    const relationshipId = params.get("relationship") || "";
    if (!nodeId && !relationshipId) return null;
    return { nodeId, relationshipId };
  }

  function resolvePendingLocationTarget() {
    if (!pendingLocationTarget) return false;
    const target = pendingLocationTarget;
    pendingLocationTarget = null;
    window.setTimeout(() => {
      navigateToGraphTarget(target, { updateUrl: false }).catch((err) => {
        statusEl.textContent = err.message || "Graph target unavailable";
      });
    }, 0);
    return true;
  }

  async function navigateToGraphTarget(target, options = {}) {
    await ensureGraphTarget(target);
    applyGraphFilters(true);

    const relationship = target.relationshipId ? findGraphRelationship(target.relationshipId) : null;
    if (relationship) {
      const focusNodeId = target.nodeId || relationshipEndpoint(relationship.target);
      const item = { kind: "relationship", value: relationship, focusNodeId };
      selectItem(item, options);
      snapRelationshipIntoView(relationship);
      return;
    }

    const node = target.nodeId ? findGraphNode(target.nodeId) : null;
    if (node) {
      selectItem({ kind: "node", value: node }, options);
      snapNodeIntoView(node);
    }
  }

  async function ensureGraphTarget(target) {
    let details = null;
    let changed = false;
    let filtersChanged = false;

    if (target.relationship) {
      changed = mergeGraphRelationship(target.relationship, { persist: false }) || changed;
    }

    if (target.nodeId && !findFullGraphNode(target.nodeId)) {
      details = await fetchNodeDetails(target.nodeId);
      changed = mergeGraphNode(details, { persist: false }) || changed;
    }

    let relationship = target.relationshipId ? findFullGraphRelationship(target.relationshipId) : null;
    if (!relationship && target.relationshipId && target.nodeId) {
      if (!details) details = await fetchNodeDetails(target.nodeId);
      relationship = (details.relationships || []).find((rel) => relationshipId(rel) === target.relationshipId);
      if (relationship) {
        changed = mergeGraphRelationship(relationship, { persist: false }) || changed;
      }
    }

    if (relationship) {
      const sourceNode = await ensureGraphNode(relationshipEndpoint(relationship.source));
      const targetNode = await ensureGraphNode(relationshipEndpoint(relationship.target));
      filtersChanged = allowNodeFilters(sourceNode) || filtersChanged;
      filtersChanged = allowNodeFilters(targetNode) || filtersChanged;
      filtersChanged = allowPredicateFilter(relationship.type) || filtersChanged;
    }

    const node = target.nodeId ? findFullGraphNode(target.nodeId) : null;
    filtersChanged = allowNodeFilters(node) || filtersChanged;

    if (changed || filtersChanged) {
      if (changed) {
        materializeFullGraph();
        scheduleGraphCacheSave();
      }
      if (filtersChanged) {
        saveStoredFilters();
        renderFilterGroup(labelFiltersEl, "labels", sortedUnique([...filters.labels.keys()]));
        renderFilterGroup(predicateFiltersEl, "predicates", sortedUnique([...filters.predicates.keys()]));
        syncFilterGroupControl("labels");
        syncFilterGroupControl("predicates");
      } else {
        syncFilterControls();
      }
    }
  }

  async function ensureGraphNode(id) {
    if (!id) return null;
    const existing = findFullGraphNode(id);
    if (existing) return existing;
    const details = await fetchNodeDetails(id);
    mergeGraphNode(details, { persist: false });
    materializeFullGraph();
    scheduleGraphCacheSave();
    return findFullGraphNode(id);
  }

  function mergeGraphNode(node, options = {}) {
    if (!node?.id) return false;
    const existing = graphStore.nodes.get(node.id);
    const next = {
      id: node.id,
      labels: sortedUnique([...(existing?.labels || []), ...(node.labels || [])]),
      properties: { ...(existing?.properties || {}), ...(node.properties || {}) },
    };
    if (Array.isArray(node.relationships)) {
      next.relationships = node.relationships.map(serializeCachedRelationship);
      next.detailsCached = true;
    } else if (Array.isArray(existing?.relationships)) {
      next.relationships = existing.relationships;
    }
    if (existing?.detailsCached) next.detailsCached = true;
    const old = existing || nodeState.get(node.id);
    if (old) {
      next.x = old.x;
      next.y = old.y;
      next.vx = old.vx;
      next.vy = old.vy;
      next.fx = old.fx;
      next.fy = old.fy;
    }
    if (existing) {
      const changed = stableStringify(serializeCachedNode(existing)) !== stableStringify(serializeCachedNode(next));
      Object.assign(existing, next);
      nodeState.set(existing.id, existing);
      if (changed && options.persist !== false) {
        materializeFullGraph();
        scheduleGraphCacheSave();
      }
      return changed;
    }
    graphStore.nodes.set(next.id, next);
    nodeState.set(next.id, next);
    if (options.persist !== false) {
      materializeFullGraph();
      scheduleGraphCacheSave();
    }
    return true;
  }

  function mergeGraphRelationship(rel, options = {}) {
    if (!rel) return false;
    const id = relationshipId(rel);
    const existing = graphStore.relationships.get(id);
    const next = {
      id: relationshipId(rel),
      source: relationshipEndpoint(rel.source),
      target: relationshipEndpoint(rel.target),
      type: rel.type,
      properties: { ...(existing?.properties || {}), ...(rel.properties || {}) },
    };
    if (existing) {
      const changed = stableStringify(serializeCachedRelationship(existing)) !== stableStringify(serializeCachedRelationship(next));
      Object.assign(existing, next);
      if (changed && options.persist !== false) {
        materializeFullGraph();
        scheduleGraphCacheSave();
      }
      return changed;
    }
    graphStore.relationships.set(id, next);
    if (options.persist !== false) {
      materializeFullGraph();
      scheduleGraphCacheSave();
    }
    return true;
  }

  function allowNodeFilters(node) {
    if (!node) return false;
    return (node.labels || []).reduce((changed, label) => {
      if (filters.labels.get(label) === true) return changed;
      filters.labels.set(label, true);
      return true;
    }, false);
  }

  function allowPredicateFilter(type) {
    if (!type || filters.predicates.get(type) === true) return false;
    filters.predicates.set(type, true);
    return true;
  }

  async function fetchNodeDetails(id, options = {}) {
    const cached = cachedNodeDetails(id);
    if (cached && (!options.requireComplete || cachedNodeDetailsComplete(cached))) return cached;
    const response = await fetch(`/graph/node/${encodeURIComponent(id)}`, { cache: "force-cache" });
    if (!response.ok) throw new Error(`detail request failed: ${response.status}`);
    const details = await response.json();
    mergeGraphNode(details, { persist: false });
    (details.relationships || []).forEach((rel) => mergeGraphRelationship(rel, { persist: false }));
    materializeFullGraph();
    scheduleGraphCacheSave();
    return details;
  }

  function cachedNodeDetails(id) {
    const node = graphStore.nodes.get(id);
    if (!node?.detailsCached) return null;
    return {
      id: node.id,
      labels: node.labels || [],
      properties: node.properties || {},
      relationships: node.relationships || [],
    };
  }

  function cachedNodeDetailsComplete(node) {
    const media = mediaForNode(node);
    if (nodeKind(node) === "SpeechSegment") return true;
    return !media.mime || !!media.base64;
  }

  function findGraphNode(id) {
    return graph.nodes.find((node) => node.id === id);
  }

  function findFullGraphNode(id) {
    return graphStore.nodes.get(id) || null;
  }

  function findGraphRelationship(id) {
    return graph.relationships.find((rel) => relationshipId(rel) === id);
  }

  function findFullGraphRelationship(id) {
    return graphStore.relationships.get(id) || null;
  }

  function graphTargetHref(target) {
    const url = new URL(window.location.href);
    if (target.nodeId) {
      url.searchParams.set("node", target.nodeId);
    } else {
      url.searchParams.delete("node");
    }
    if (target.relationshipId) {
      url.searchParams.set("relationship", target.relationshipId);
    } else {
      url.searchParams.delete("relationship");
    }
    return `${url.pathname}${url.search}${url.hash}`;
  }

  function updateUrlForSelection(item, options = {}) {
    if (item.kind === "node") {
      updateGraphTargetUrl({ nodeId: item.value.id }, options);
    } else {
      updateGraphTargetUrl(
        {
          nodeId: item.focusNodeId || relationshipEndpoint(item.value.target),
          relationshipId: relationshipId(item.value),
        },
        options,
      );
    }
  }

  function updateGraphTargetUrl(target, options = {}) {
    const next = graphTargetHref(target);
    const current = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    if (next === current) return;
    window.history[options.replaceUrl ? "replaceState" : "pushState"]({}, "", next);
  }

  function clearGraphTargetUrl(options = {}) {
    const url = new URL(window.location.href);
    url.searchParams.delete("node");
    url.searchParams.delete("relationship");
    const next = `${url.pathname}${url.search}${url.hash}`;
    const current = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    if (next === current) return;
    window.history[options.replaceUrl ? "replaceState" : "pushState"]({}, "", next);
  }

  function snapNodeIntoView(node) {
    const point = endpoint(node.id);
    snapPointsIntoView([{ x: point.x, y: point.y }], 2.2);
  }

  function snapRelationshipIntoView(rel) {
    const source = endpoint(rel.source);
    const target = endpoint(rel.target);
    snapPointsIntoView(
      [
        { x: source.x, y: source.y },
        { x: target.x, y: target.y },
      ],
      1.85,
    );
  }

  function snapPointsIntoView(points, maxScale) {
    const rect = svg.node().getBoundingClientRect();
    if (!points.length || rect.width === 0 || rect.height === 0) return;
    const xs = points.map((point) => point.x ?? rect.width / 2);
    const ys = points.map((point) => point.y ?? rect.height / 2);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const width = Math.max(maxX - minX, 1);
    const height = Math.max(maxY - minY, 1);
    const scale = Math.max(0.15, Math.min(maxScale, 0.7 / Math.max(width / rect.width, height / rect.height)));
    const tx = rect.width / 2 - scale * (minX + width / 2);
    const ty = rect.height / 2 - scale * (minY + height / 2);
    svg.transition().duration(260).call(zoom.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
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
    scheduleGraphCacheSave();
  }

  function resize() {
    const rect = svg.node().getBoundingClientRect();
    simulation.force("center", d3.forceCenter(rect.width / 2, rect.height / 2));
    simulation.force("theme-x").x(rect.width / 2);
    simulation.force("theme-y").y(rect.height / 2);
    simulation.force("time-x").x(temporalX);
    simulation.alpha(0.3).restart();
    updateTimelinePlayhead();
    renderTimelineRuler();
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

  svg.on("click", clearSelection);

  graphModeEl.addEventListener("click", () => setViewMode("graph"));
  timelineModeEl.addEventListener("click", () => setViewMode("timeline"));
  timelineScrubEl.addEventListener("input", () => {
    if (!timelineExtent) return;
    const ratio = clamp01(Number(timelineScrubEl.value) / Number(timelineScrubEl.max || 1000));
    timelineCursor = timelineExtent.min + ratio * (timelineExtent.max - timelineExtent.min);
    syncTimelineScrubber();
  });
  timelinePlayEl.addEventListener("click", playTimeline);
  timelinePauseEl.addEventListener("click", pauseTimeline);
  timelineZoomInEl.addEventListener("click", () => zoomTimelineBy(0.5));
  timelineZoomOutEl.addEventListener("click", () => zoomTimelineBy(2));
  timelineZoomResetEl.addEventListener("click", resetTimelineZoom);
  timelineBoardEl.addEventListener("pointerdown", startTimelineSelection);
  timelineBoardEl.addEventListener("pointermove", moveTimelineSelection);
  timelineBoardEl.addEventListener("pointerup", finishTimelineSelection);
  timelineBoardEl.addEventListener("pointercancel", cancelTimelineSelection);
  document.getElementById("zoom-in").addEventListener("click", () => zoomBy(1.25));
  document.getElementById("zoom-out").addEventListener("click", () => zoomBy(0.8));
  document.getElementById("zoom-fit").addEventListener("click", fitGraph);
  allLabelFiltersEl?.addEventListener("change", () => setFilterGroup("labels", allLabelFiltersEl.checked));
  allPredicateFiltersEl?.addEventListener("change", () =>
    setFilterGroup("predicates", allPredicateFiltersEl.checked),
  );
  window.addEventListener("resize", resize);
  window.addEventListener("popstate", () => {
    const target = targetFromLocation();
    if (target) {
      navigateToGraphTarget(target, { updateUrl: false }).catch((err) => {
        statusEl.textContent = err.message || "Graph target unavailable";
      });
    } else {
      clearSelection({ updateUrl: false });
    }
  });

  resize();
})();
