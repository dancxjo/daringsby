export interface GeoLoc {
  longitude: number;
  latitude: number;
}

export interface AudioData {
  base64: string;
  mime: string;
}

export interface WitReport {
  name: string;
  prompt: string;
  output: string;
}

export type WsMessage =
  | { type: "Say"; data: { words: string; audio?: string | null } }
  | { type: "Emote"; data: string }
  | { type: "Think"; data: WitReport }
  | { type: "Text"; text: string }
  | { type: "Echo"; text: string }
  | { type: "See"; data: string; at?: string }
  | { type: "Hear"; data: AudioData; at?: string }
  | { type: "Geolocate"; data: GeoLoc; at?: string }
  | { type: "Sense"; data: Record<string, any> };
