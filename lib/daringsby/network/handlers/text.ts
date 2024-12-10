import logger from "../../core/logger.ts";
import { isValidTextMessage } from "../messages/TextMessage.ts";
import { Session } from "../Sessions.ts";
// import { Image } from "../../vision/describer.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingTextMessages(session: Session) {
  //   logger.debug("Ignoring vision for now");
  session.subscriptions.push(
    session.connection.incoming(isValidTextMessage).subscribe(
      async (message) => {
        logger.debug("Received a valid SeeMessage");
        psyche.hear({
          role: "user",
          content: message.data,
        });
      },
    ),
  );
}

export default handleIncomingTextMessages;
