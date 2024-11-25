import { SocketConnection } from "./sockets/connection.ts";
import { ReplaySubject, Subject, Subscription, timer } from "npm:rxjs";
import { switchMap } from "npm:rxjs/operators";
import { Sensation } from "../core/interfaces.ts";
import { errorSubject, logger } from "../core/logger.ts";
import {
  handleEchoes,
  handleGeolocations,
  handleIncomingTexts,
  handleVision,
  setupHeartbeat,
} from "./sockets/handlers.ts";
import { Voice } from "../genii/Voice.ts";
import { establishMemory } from "../utils/memory.ts";
import { Genie } from "../genii/Genie.ts";
import { narrate } from "../utils/narration.ts";
import { MessageType } from "./messages/MessageType.ts";

export class Session {
  protected latestSituation$: ReplaySubject<Sensation<string>> =
    new ReplaySubject<Sensation<unknown>>(1);
  protected thoughts$: Subject<string> = new Subject<string>();
  readonly voice = new Voice("Voice", this.latestSituation$, this);
  protected timeline: Sensation<unknown>[] = [];
  protected spinning = true;
  readonly integration = new Genie(
    "Binding activities",
    "You are responsible for binding together all the disparate senses into an integrated understanding of the current moment: what's going on and what's happened to get you here.",
    "Read the timeline of events below. They appear in chronological order and are of sundry grains of detail. Your job is to progressively condense then down to natural language while preserving order. The further an event is on the timeline, the less relevant it is to the current context, so the head of your narration should be a general summary of things since you began. Then gradually, as events approach the current moment, more and more salient details are introduced. These events occur asynchronously and you will need to mix levels of granularity. Narrate in the first person, as if you were the one experiencing them. Do not embellish or invent new details; just go by the details here. Sensations: {{#sensations}}\nAt {{when}}, {{content.explanation}}\n{{/sensations}}\n\nBe succinct. Don't repeat yourself. Don't add new details. Just narrate what's here. Make sure to rearrange the sensations in chronological order in the description. Do not preface your answer; just dive right in to the narration. Also bear in mind that your senses will deceive you, so be prepared to correct yourself. The image your seeing is from your own point of view, so you're probably seeing your interlocutor, not yourself, unless you're looking in a mirror.",
    narrate,
  );

  constructor(
    readonly connection: SocketConnection,
    readonly subscriptions: Subscription[],
  ) {
    // Subscribe to errors from the logger
    const errorSubscription = errorSubject.subscribe((sensation) => {
      this.feel(sensation);
    });

    // Add this subscription to the list so it can be managed properly
    this.subscriptions.push(errorSubscription);
    establishMemory();
    handleVision(this);
    handleGeolocations(this);
    handleEchoes(this);
    handleIncomingTexts(this);
    setupHeartbeat(this);

    this.subscriptions.push(this.latestSituation$.subscribe((sensation) => {
      logger.debug({ sensation }, "Received latest situation");
    }));
  }

  feel(sensation: Sensation<unknown>) {
    logger.debug({ sensation }, "Feeling sensation");
    this.timeline.push(sensation);
    this.timeline.sort((a, b) => a.when.getTime() - b.when.getTime());
    this.integration.feel(sensation);
  }

  async spin() {
    // Start both the voice and integration processing independently
    this.processVoice();
    // this.processIntegration();
  }

  async processVoice() {
    while (this.spinning) {
      try {
        const nextThought = await this.voice.consult().toPromise();
        if (nextThought) {
          this.thoughts$.next(nextThought);
          this.feel({
            when: new Date(),
            content: {
              explanation: `I just thought something: ${nextThought}`,
              content: nextThought,
            },
          });
        }
      } catch (error) {
        logger.error({ error }, "Error during voice processing");
      }
      // Avoid tight looping, add a slight delay
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  async processIntegration() {
    while (this.spinning) {
      try {
        const present = await this.integration.consult().toPromise();
        if (present) {
          const summary = {
            when: new Date(),
            content: {
              explanation: present,
              content: present,
            },
          };
          this.latestSituation$.next(summary);
          this.feel(summary);
          this.connection.send({
            type: MessageType.Think,
            data: present,
          });
        }
      } catch (error) {
        logger.error({ error }, "Error during integration processing");
      }
      // Avoid tight looping, add a slight delay
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }

  stop() {
    this.spinning = false;
  }

  unsubscribe() {
    this.subscriptions.forEach((subscription) => subscription.unsubscribe());
  }
}

export const sessions = new Map<WebSocket, Session>();

export function addSession(
  socket: WebSocket,
  connection: SocketConnection,
): Session {
  const session = new Session(
    connection,
    [],
  );

  sessions.set(socket, session);
  session.spin();
  return session;
}

export function removeSession(socket: WebSocket) {
  const session = sessions.get(socket);
  if (session) {
    session.stop();
    session.connection.hangup();
    session.unsubscribe();
    sessions.delete(socket);
  }
}
