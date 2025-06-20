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
        ).catch((error) => {
          logger.error(
            {
              error,
              latitude: message.data.latitude,
              longitude: message.data.longitude,
            },
            "Error fetching address from geolocation",
          );
          return "an unknown location";
        });
        const impression = {
          how:
            `I am geolocated *near* ${message.data.latitude}, ${message.data.longitude}. According to my reverse geocoding, I am near ${
              JSON.stringify(address)
            }. As is usual for geolocations, this may flicker around a bit and get more accurate over time.`,
          when: new Date(),
        };
        psyche.witness(impression);
        return impression;
      },
    ),
  );
}

export default handleIncomingGeolocationMessages;
