import { logger } from "../../core/logger.ts";
import { Session } from "../Sessions.ts";
import { isValidGeolocateMessage } from "../messages/GeolocateMessage.ts";
import { map } from "npm:rxjs/operators";
import { isValidEchoMessage } from "../messages/EchoMessage.ts";
import { isValidTextMessage } from "../messages/TextMessage.ts";
import * as yaml from "npm:yaml";
import { see } from "../../utils/narration.ts";
import { isValidSeeMessage } from "../messages/SeeMessage.ts";

export function setupHeartbeat(session: Session) {
  setInterval(() => {
    const now = new Date();
    session.feel({
      when: now,
      content: {
        explanation:
          `${now.toISOString()} is the current time. I feel my heart beat.`,
        content: now.toISOString(),
      },
    });
  }, 3000);
}

export function handleVision(session: Session) {
  const vision$ = session.connection.incoming(isValidSeeMessage).pipe(
    map((message) => {
      see(message.data);
      return {
        when: new Date(message.at ?? new Date()),
        content: {
          explanation: "I just saw something",
          content: message.data,
        },
      };
    }),
  );
  session.subscriptions.push(vision$.subscribe((sensation) => {
    session.feel(sensation);
    logger.debug("Processed vision sensation");
  }));
}

export function handleGeolocations(
  session: Session,
) {
  const geolocation$ = session.connection.incoming(isValidGeolocateMessage)
    .pipe(
      map((message) => {
        logger.debug({ message }, "Received geolocation");
        return {
          when: new Date(message.at ?? new Date()),
          content: {
            explanation:
              `I am physically located at ${message.data.latitude}, ${message.data.longitude}`,
            content: message.data,
          },
        };
      }),
    );
  session.subscriptions.push(geolocation$.subscribe((sensation) => {
    session.feel({
      when: sensation.when,
      content: {
        explanation: sensation.content.explanation,
        content: yaml.stringify(sensation.content.content),
      },
    });
    logger.debug({ sensation }, "Processed geolocation sensation");
  }));
}

export function handleEchoes(
  session: Session,
) {
  const echoes$ = session.connection.incoming(isValidEchoMessage)
    .pipe(
      map((message) => {
        logger.debug({ message }, "Received echo");
        return {
          when: new Date(message.at ?? new Date()),
          content: {
            explanation: `I just heard myself finish saying: ${message.data}`,
            content: message.data,
          },
        };
      }),
    );
  session.subscriptions.push(echoes$.subscribe((sensation) => {
    session.feel({
      when: sensation.when,
      content: {
        explanation: sensation.content.explanation,
        content: sensation.content.content,
      },
    });
    session.voice.echo(sensation.content.content);
    logger.info({ sensation }, "Processed echo sensation");
  }));
}

export function handleIncomingTexts(
  session: Session,
) {
  const text$ = session.connection.incoming(isValidTextMessage)
    .pipe(
      map((message) => {
        logger.debug({ message }, "Received text");
        return {
          when: new Date(message.at ?? new Date()),
          content: {
            explanation: `I just heard my interlocutor say: ${message.data}`,
            content: message.data,
          },
        };
      }),
    );
  session.subscriptions.push(text$.subscribe((sensation) => {
    session.feel({
      when: sensation.when,
      content: {
        explanation: sensation.content.explanation,
        content: sensation.content.content,
      },
    });
    session.voice.hear(sensation.content.content);
    logger.debug({ sensation }, "Processed text sensation");
  }));
}
