import { Sensation, Sensitive } from "../core/interfaces.ts";

export interface Image {
  base64: string;
}

export class ImageDescriber implements Sensitive<Image> {
  feel(sensation: Sensation<Image>) {
    return {
      how: "This is an image",
      what: sensation,
    };
  }
}
