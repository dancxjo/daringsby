export interface GeoLoc {
  longitude: number;
  latitude: number;
  observed_at?: string;
}

export interface MotionVector {
  x?: number;
  y?: number;
  z?: number;
}

export interface DeviceOrientation {
  alpha?: number;
  beta?: number;
  gamma?: number;
  absolute?: boolean;
}

export interface BrowserMotion {
  acceleration?: MotionVector;
  acceleration_including_gravity?: MotionVector;
  rotation_rate?: DeviceOrientation;
  orientation?: DeviceOrientation;
  interval?: number;
  observed_at?: string;
}

export interface AudioData {
  base64: string;
  mime: string;
  sample_rate?: number;
  channels?: number;
}

export interface WitReport {
  name: string;
  prompt: string;
  output: string;
}

export interface WillTypeScriptResult {
  command: string;
  output: string;
}

export interface WillTypeScriptExecution {
  source: string;
  timestamp: string;
  results: WillTypeScriptResult[];
}

export interface WillContext {
  system_prompt: string;
  history: ConversationEntry[];
  report?: WitReport | null;
  typescript?: WillTypeScriptExecution | null;
}

export interface ConversationEntry {
  role: string;
  content: string;
  timestamp: string;
}

export type WsMessage =
  | { type: "Say"; data: { words: string; audio?: string | null } }
  | { type: "Emote"; data: string }
  | { type: "Think"; data: WitReport }
  | { type: "Text"; data: { text: string; at?: string } }
  | { type: "Echo"; text: string; at?: string }
  | { type: "See"; data: string; at?: string }
  | { type: "Hear"; data: AudioData; at?: string }
  | { type: "Geolocate"; data: GeoLoc; at?: string }
  | { type: "Motion"; data: BrowserMotion; at?: string }
  | { type: "Sense"; data: Record<string, any> }
  | { type: "SystemPrompt"; data: string }
  | { type: "ConversationEntry"; data: ConversationEntry }
  | { type: "FullHistory"; data: WillContext };
