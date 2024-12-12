import logger from "../../core/logger.ts";
import { isValidGeolocateMessage } from "../messages/GeolocateMessage.ts";
import { Session } from "../Sessions.ts";
import { psyche } from "../../core/psyche.ts";
import { getAddressByLocation } from "npm:@fs-fns/reverse-geocode";

export function handleIncomingGeolocationMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidGeolocateMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a GeolocationMessage");
        const address = await getAddressByLocation(
          message.data.latitude,
          message.data.longitude,
        );
        const impression = {
          how:
            `I am geolocated at ${message.data.latitude}, ${message.data.longitude}. According to my reverse geocoding, I am at or near ${
              JSON.stringify(address)
            }.`,
          when: new Date(),
        };
        psyche.witness(impression);
        return impression;
      },
    ),
  );
}

export default handleIncomingGeolocationMessages;
