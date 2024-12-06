import logger from "../../core/logger.ts";
import { isValidSeeMessage } from "../messages/SeeMessage.ts";
import { Session } from "../Sessions.ts";
import { Image } from "../../vision/describer.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingSeeMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidSeeMessage).subscribe(
      async (message) => {
        logger.debug("Received a valid SeeMessage");
        const image: Image = { base64: message.data };
        const impression = await psyche.see({
          when: new Date(message.at),
          what: image,
        });
        psyche.witness(impression);
        return impression;
      },
    ),
  );
}

export default handleIncomingSeeMessages;
