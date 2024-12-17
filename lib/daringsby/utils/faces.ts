export interface FaceDetectionResponse {
  level: number;
  time: number;
  pid: number;
  hostname: string;
  name: string;
  faces: {
    result: FaceResult[];
    plugins_versions: PluginsVersions;
  };
  msg: string;
}

export interface FaceResult {
  age: {
    probability: number;
    high: number;
    low: number;
  };
  gender: {
    probability: number;
    value: string;
  };
  embedding: number[];
  box: {
    probability: number;
    x_max: number;
    y_max: number;
    x_min: number;
    y_min: number;
  };
  landmarks: [number, number][];
  execution_time: {
    age: number;
    gender: number;
    detector: number;
    calculator: number;
  };
}

export interface PluginsVersions {
  age: string;
  gender: string;
  detector: string;
  calculator: string;
}
//
const apiKey = "39636853-48a5-4a93-9e03-92d06a21279c";
const url =
  "http://forebrain.local:8008/api/v1/detection/detect?limit=0&det_prob_threshold=0.8&face_plugins=calculator,age,gender,landmarks&status=true";

const recognitionApiKey = "562b2d13-8723-45c4-98a1-c69ab0bccd90";
const recognitionUrl =
  "http://forebrain.local:8008/api/v1/recognition/recognize?face_plugins=landmarks,gender,age,pose";

async function postJsonRequest(
  apiUrl: string,
  apiKey: string,
  base64Image: string,
) {
  try {
    const payload = {
      file: base64Image, // Send raw Base64 data as "file" in JSON
    };

    const response = await fetch(apiUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-api-key": apiKey,
      },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `HTTP Error: ${response.status} - ${JSON.stringify(errorData)}`,
      );
    }

    return await response.json();
  } catch (error) {
    console.error("Error making request:", error);
    return null;
  }
}

export async function detectFaces(base64Image: string) {
  const data = await postJsonRequest(url, apiKey, base64Image);
  console.log("Face Detection Response:", data);
  return data as FaceDetectionResponse;
}

export async function recognizeFaces(base64Image: string) {
  const data = await postJsonRequest(
    recognitionUrl,
    recognitionApiKey,
    base64Image,
  );
  console.log("Face Recognition Response:", data);
  return data;
}

export function describeFace(result: FaceResult): string {
  try {
    const age = result?.age ?? {};
    const gender = result?.gender ?? {};
    const box = result?.box ?? {};
    const landmarks = result?.landmarks ?? [];
    const execution_time = result?.execution_time ?? {};

    // Age description
    const ageLow = age.low ?? "N/A";
    const ageHigh = age.high ?? "N/A";
    const ageProb = age.probability != null
      ? (age.probability * 100).toFixed(2)
      : "N/A";
    const ageDescription =
      `Estimated age range: ${ageLow} - ${ageHigh} (confidence: ${ageProb}%)`;

    // Gender description
    const genderValue = gender.value ?? "Unknown";
    const genderProb = gender.probability != null
      ? (gender.probability * 100).toFixed(2)
      : "N/A";
    const genderDescription =
      `Gender detected: ${genderValue} (confidence: ${genderProb}%)`;

    // Box description
    const x_min = box.x_min ?? "N/A";
    const y_min = box.y_min ?? "N/A";
    const x_max = box.x_max ?? "N/A";
    const y_max = box.y_max ?? "N/A";
    const boxDescription =
      `Bounding box: Top-left (${x_min}, ${y_min}), Bottom-right (${x_max}, ${y_max})`;

    // Landmarks description
    const landmarksDescription = landmarks.length
      ? landmarks
        .map(([x, y], index) =>
          `Landmark ${index + 1}: (${x ?? "N/A"}, ${y ?? "N/A"})`
        )
        .join("; ")
      : "No landmarks detected";

    // Execution time
    const executionTimeDescription = `
      Execution times (ms):
      - Age: ${execution_time.age ?? "N/A"}
      - Gender: ${execution_time.gender ?? "N/A"}
      - Detector: ${execution_time.detector ?? "N/A"}
      - Calculator: ${execution_time.calculator ?? "N/A"}
      `;

    // Combine all descriptions
    return [
      "Face Detection Result:",
      ageDescription,
      genderDescription,
      boxDescription,
      `Detected landmarks: ${landmarksDescription}`,
      executionTimeDescription,
    ].join("\n");
  } catch (error) {
    console.error("Error describing face:", error);
    return "Error describing face: Insufficient or invalid data.";
  }
}
