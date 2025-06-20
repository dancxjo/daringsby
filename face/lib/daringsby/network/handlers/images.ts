import logger from "../../core/logger.ts";
import { isValidSeeMessage } from "../messages/SeeMessage.ts";
import { Session } from "../Sessions.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingSeeMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidSeeMessage).subscribe(
      async (message) => {
        psyche.see(message.data);
      },
    ),
  );
}

export default handleIncomingSeeMessages;
