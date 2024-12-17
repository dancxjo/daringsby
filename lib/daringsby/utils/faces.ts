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

const apiKey = "39636853-48a5-4a93-9e03-92d06a21279c";
const url =
  "http://forebrain.local:8008/api/v1/detection/detect?limit=0&det_prob_threshold=0.8&face_plugins=calculator,age,gender,landmarks&status=true";

export async function detectFaces(base64Image: string) {
  try {
    // Prepare the JSON payload
    const payload = {
      file: base64Image, // Send raw Base64 data as "file" in JSON
    };

    // Make the POST request using fetch
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-api-key": apiKey,
      },
      body: JSON.stringify(payload), // Stringify the JSON payload
    });

    // Handle response
    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `HTTP Error: ${response.status} - ${JSON.stringify(errorData)}`,
      );
    }

    const data = await response.json();
    console.log("Face Detection Response:", data);
    return data as FaceDetectionResponse;
  } catch (error) {
    console.error("Error detecting faces:", error);
  }
}

export function describeFace(result: FaceResult): string {
  const {
    age: { probability: ageProb, high: ageHigh, low: ageLow },
    gender: { probability: genderProb, value: genderValue },
    box: { x_min, y_min, x_max, y_max },
    landmarks,
    execution_time,
  } = result;

  // Age description
  const ageDescription =
    `Estimated age range: ${ageLow} - ${ageHigh} (confidence: ${
      (ageProb * 100).toFixed(2)
    }%)`;

  // Gender description
  const genderDescription = `Gender detected: ${genderValue} (confidence: ${
    (genderProb * 100).toFixed(2)
  }%)`;

  // Box description
  const boxDescription =
    `Bounding box: Top-left (${x_min}, ${y_min}), Bottom-right (${x_max}, ${y_max})`;

  // Landmarks description
  const landmarksDescription = landmarks
    .map(([x, y], index) => `Landmark ${index + 1}: (${x}, ${y})`)
    .join("; ");

  // Execution time
  const executionTimeDescription = `
  Execution times (ms): 
  - Age: ${execution_time.age}
  - Gender: ${execution_time.gender}
  - Detector: ${execution_time.detector}
  - Calculator: ${execution_time.calculator}
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
}
