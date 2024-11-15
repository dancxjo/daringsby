import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

interface GeolocateMessage extends SocketMessage {
    type: MessageType.Say;
    data: {
        longitude: number;
        latitude: number;
    };
}

export function isValidGeolocateMessage(
    m: SocketMessage,
): m is GeolocateMessage {
    return m.type === MessageType.Geolocate && typeof m.data === "object" &&
        "longitude" in m?.data && typeof m.data.longitude === "number" &&
        typeof m.data.latitude === "number";
}
