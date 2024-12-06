import { Session } from "../Sessions.ts";
import logger from "../../core/logger.ts";
import { isValidTextMessage } from "../messages/TextMessage.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingTextMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidTextMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a TextMessage");
        const impression = {
          how: `I just heard someone say to me: ${message.data}`,
          depth_low: 0,
          depth_high: 0,
          what: {
            when: new Date(),
            what: message.data,
          },
        };
        psyche.witness(impression);
        return impression;
      },
    ),
  );
}

export default handleIncomingTextMessages;
