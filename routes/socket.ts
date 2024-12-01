import { Handlers } from "$fresh/server.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import {
  addSession,
  Session,
  sessions,
} from "../lib/daringsby/network/Sessions.ts";
import { logger } from "../logger.ts";
import { isValidSeeMessage } from "../lib/daringsby/network/messages/SeeMessage.ts";
import { Image } from "../lib/daringsby/vision/describer.ts";
import { ImageDescriber } from "../lib/daringsby/vision/describer.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import { isValidSenseMessage } from "../lib/daringsby/network/messages/SenseMessage.ts";
import {
  Experiencer,
  Impression,
  Sensation,
} from "../lib/daringsby/core/interfaces.ts";
import { lm } from "../lib/daringsby/core/core.ts";
import { isValidTextMessage } from "../lib/daringsby/network/messages/TextMessage.ts";

export const handler: Handlers = {
  GET(req, _ctx) {
    logger.debug("Received GET request");
    if (!req.headers.get("upgrade")?.toLowerCase().includes("websocket")) {
      logger.error("Received non-WebSocket request");
      return new Response("Expected WebSocket request", { status: 400 });
    }
    const { socket, response } = Deno.upgradeWebSocket(req);

    if (!socket) {
      logger.error("Failed to upgrade to WebSocket");
      return response;
    }

    if (!sessions.has(socket)) {
      logger.debug("Creating new SocketToClient for WebSocket");
      const connection = new SocketConnection(socket);
      addSession(socket, connection);
    }

    const session = sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    handleIncomingSeeMessages(session);
    handleIncomingSenseMessages(session);
    handleIncomingTextMessages(session);

    logger.debug("Successfully upgraded to WebSocket");
    return response;
  },
};

class Witness implements Experiencer {
  protected impressions: Impression<unknown>[] = [];
  protected lastTick: number = Date.now();

  enqueue(impression: Impression): void {
    this.impressions.push(impression);
  }

  async feel(
    sensation: Sensation<Impression[]>,
  ): Promise<Impression<Impression<unknown>[]>> {
    this.impressions = [...this.impressions, ...sensation.what];
    this.impressions.sort((a, b) =>
      a.what.when.getTime() - b.what.when.getTime()
    );

    const prompt =
      `You are the linguistic processor of an embodied artificial being. These are the impressions of the sensations you have recently felt:\n\n${
        this.impressions.map((impression) =>
          `${impression.what.when.toLocaleString()}: ${impression.how}\n`
        ).join("\n")
      }. Generate a rich narration of the experience from the perspective of the artificial being. Narrate in the first person on behalf of the artificial being. Be succinct. Edit out irrelevant details and highlight the salient ones. Merge related events into narratives. Let's imagine you were to feel the keys spell something out. Don't invent events; just try to piece together the given events into a logical explanation. Connect events together. If you see someone, they might be the same someone you feel pressing your keys; they might be trying to communicate with you.`;

    logger.info({ prompt }, "Generating experience");

    const experience = await lm.generate({
      prompt,
    });

    const rv = {
      how: experience,
      what: {
        when: new Date(),
        what: this.impressions,
      },
    };

    // Scroll older events off the list
    // TODO: Vectorize and memorize the impressions
    this.impressions = this.impressions.filter((impression) =>
      impression.what.when.getTime() > Date.now() - 1000 * 60 * 3
    );

    return rv;
  }
}

const witness = new Witness();

function tick() {
  setTimeout(async () => {
    const impression = await witness.feel({
      when: new Date(),
      what: [],
    });
    sessions.forEach((session) => {
      session.connection.send({
        type: MessageType.Think,
        data: impression.how,
      });
    });
    tick();
  }, 1000);
}

tick();

function handleIncomingSeeMessages(session: Session) {
  const eye = new ImageDescriber();

  session.subscriptions.push(
    session.connection.incoming(isValidSeeMessage).subscribe(
      async (message) => {
        logger.debug("Received a valid SeeMessage");
        const image: Image = { base64: message.data };
        const impression = await eye.feel({
          when: new Date(message.at),
          what: image,
        });
        witness.enqueue(impression);
        return impression;
      },
    ),
  );
}

function handleIncomingSenseMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidSenseMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a valid SenseMessage");
        witness.enqueue({
          ...message.data,
          what: {
            ...message.data.what,
            when: new Date(message.data.what.when),
          },
        });
      },
    ),
  );
}

function handleIncomingTextMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidTextMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a TextMessage");
        const impression = {
          how: `I heard: ${message.data}`,
          what: {
            when: new Date(),
            what: message.data,
          },
        };
        witness.enqueue(impression);
        return impression;
      },
    ),
  );
}
