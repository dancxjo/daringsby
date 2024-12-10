import logger from "../../core/logger.ts";
import { isValidGeolocateMessage } from "../messages/GeolocateMessage.ts";
import { Session } from "../Sessions.ts";
import { psyche } from "../../core/psyche.ts";

export function handleIncomingGeolocationMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidGeolocateMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a GeolocationMessage");
        const impression = {
          how:
            `I am geolocated at ${message.data.latitude}, ${message.data.longitude}`,
          when: new Date(),
        };
        psyche.witness(impression);
        return impression;
      },
    ),
  );
}

export default handleIncomingGeolocationMessages;
