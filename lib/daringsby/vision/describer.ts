import { Sensation, Sensitive } from "../core/interfaces.ts";
import { lm } from "../core/core.ts";

export interface Image {
  base64: string;
}

export class ImageDescriber implements Sensitive<Image> {
  async feel(sensation: Sensation<Image>) {
    const description = await lm.generate({
      prompt:
        "Describe the image from the point of view of someone in the image itself, as if the image were the view of the person describing it.",
      image: sensation.what.base64,
    });

    const refinement = await lm.generate({
      prompt:
        `The following description is of a frame of a robot's vision. Reinterpret the description from the robot's point of view; remove all references to "the image," etc. as the image is the robot's view. Be somewhat circumspect as this is just one frame and might be deceptive, as senses sometimes are. Do not repeat the prompt or preface your answer or provide a summary at the end. Just provide the transformed description.\n\nOriginal description:\n${description}\n\nTransformed description:\n\n`,
    });
    return {
      how: refinement.trim(),
      what: sensation,
    };
  }
}
