import logger from "../../core/logger.ts";
import { isValidSenseMessage } from "../messages/SenseMessage.ts";
import { Session } from "../Sessions.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingSenseMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidSenseMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a valid SenseMessage");
        psyche.witness({
          how: `I sense: ${message.data.how}`,
          when: new Date(message.data.when),
        });
      },
    ),
  );
}

export default handleIncomingSenseMessages;
