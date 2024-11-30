import { Sensation, Sensitive } from "../core/interfaces.ts";
import { lm } from "../core/core.ts";
import logger from "../core/logger.ts";

export interface Image {
  base64: string;
}

export class ImageDescriber implements Sensitive<Image> {
  async feel(sensation: Sensation<Image>) {
    const description = await lm.generate({
      prompt: "Describe the image.",
      image: sensation.what.base64,
    });

    const refinement = await lm.generate({
      prompt:
        `The following description is of what a robot is seeing. Reinterpret it from the robot's point of view; remove any reference to 'the image' or similar, since this is what the robot is directly observing. Be somewhat cautious, as this is only one frame and could be misleading, similar to other sensory information. Do not repeat the prompt, add a preface, or provide a summary. Simply start with "I see..." and continue the description in first person as if you are the robot. You are *not* scanning an image, but directly seeing a scene! Do not refer to it as a photo or image unless it is an image of an image.

Original description:
${description}

Transformed description:

Examples:
- "This is a selfie-styled photograph of a pumpkin in a sandbank." should become "I see a pumpkin in a sandbank."
- "This is a view of a park with children playing." should become "I see a park with children playing."
- "The image shows a red car parked next to a tree." should become "I see a red car parked next to a tree."

`,
    });
    logger.info(`Refinement: ${refinement}`);
    return {
      how: refinement.trim(),
      what: sensation,
    };
  }
}
