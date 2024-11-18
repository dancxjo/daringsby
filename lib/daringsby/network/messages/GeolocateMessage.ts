import { MessageType } from "./MessageType.ts";
import { SocketMessage } from "./SocketMessage.ts";

interface GeolocateMessage extends SocketMessage {
    type: MessageType.Say;
    data: {
        longitude: number;
        latitude: number;
    };
    at: string;
}

export function isValidGeolocateMessage(
    m: SocketMessage,
): m is GeolocateMessage {
    return m.type === MessageType.Geolocate && typeof m.data === "object" &&
        "longitude" in m?.data && typeof m.data.longitude === "number" &&
        typeof m.data.latitude === "number" && "latitude" in m.data &&
        "at" in m && typeof m?.at === "string";
}
