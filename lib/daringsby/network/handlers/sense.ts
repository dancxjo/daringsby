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
          depth_low: message.data.depth_low,
          depth_high: message.data.depth_high,
          what: {
            ...message.data.what,
            when: new Date(message.data.what.when),
          },
        });
      },
    ),
  );
}

export default handleIncomingSenseMessages;
