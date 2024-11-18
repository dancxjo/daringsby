import { signal } from "@preact/signals";

export const ws = signal<WebSocket | null>(null);

export function initializeWebSocket() {
    if (
        ws.value?.readyState === WebSocket.OPEN ||
        ws.value?.readyState === WebSocket.CONNECTING
    ) {
        return;
    }
    const host = globalThis.location.host;
    const protocol = globalThis.location.protocol;
    const wsProtocol = protocol === "https:" ? "wss:" : "ws:";
    const socket = new WebSocket(`${wsProtocol}//${host}/socket`);

    socket.onopen = () => {
        console.log("WebSocket connected");
    };

    socket.onclose = () => {
        console.log("WebSocket disconnected");
    };

    ws.value = socket;
}
