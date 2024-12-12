import logger from "../../core/logger.ts";
import { isValidEchoMessage } from "../messages/EchoMessage.ts";
import { Session } from "../Sessions.ts";
// import { Image } from "../../vision/describer.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingEchoMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidEchoMessage).subscribe(
      async (message) => {
        logger.debug("Received a valid EchoMessage");

        psyche.hear({
          role: "assistant",
          content: message.data,
        });
      },
    ),
  );
}

export default handleIncomingEchoMessages;
