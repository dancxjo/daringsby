import { SocketConnection } from "./sockets/connection.ts";
import { ReplaySubject, Subject, Subscription, timer } from "npm:rxjs";
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
import { establishMemory, latestSituation, memorize } from "../utils/memory.ts";
import { Genie } from "../genii/Genie.ts";
import { narrate } from "../utils/narration.ts";
import { MessageType } from "./messages/MessageType.ts";
import { isValidSayMessage } from "./messages/SayMessage.ts";

export class Session {
  static introspection: Session;
  static thoughts$ = new Subject<string>();
  static startIntrospection() {
    if (Session.introspection) {
      return;
    }
    const connectionToSelf = new SocketConnection(
      new WebSocket("ws://localhost:8080/socket"),
    );

    connectionToSelf.incoming(isValidSayMessage).subscribe((echo) => {
      connectionToSelf.send({
        type: MessageType.Echo,
        data: echo.data.words,
        thought: true,
      });
      Session.thoughts$.next(echo.data.words);
      logger.info("Thoughts: " + echo.data.words);
    });
    const introspection = new Session(connectionToSelf, []);
    introspection.spin();
    Session.introspection = introspection;
  }

  protected latestSituation$: ReplaySubject<Sensation<string>> =
    new ReplaySubject<Sensation<unknown>>(1);
  protected speech$: Subject<string> = new Subject<string>();
  readonly voice = new Voice("Voice", this.latestSituation$, this);
  protected timeline: Sensation<unknown>[] = [];
  protected spinning = true;
  readonly integration = new Genie(
    "Binding activities",
    "You are responsible for binding together all the disparate senses into an integrated understanding of the current moment: what's going on and what's happened to get you here.",
    "Read the timeline of events below. They appear in chronological order and are of sundry grains of detail. Your job is to progressively condense then down to natural language while preserving order. The further an event is on the timeline, the less relevant it is to the current context, so the head of your narration should be a general summary of things since you began. Then gradually, as events approach the current moment, more and more salient details are introduced. These events occur asynchronously and you will need to mix levels of granularity. Narrate in the first person, as if you were the one experiencing them. Do not embellish or invent new details; just go by the details here. Sensations: {{#sensations}}\nAt {{when}}, {{content.explanation}}\n{{/sensations}}\n\nBe succinct. Don't repeat yourself. Don't add new details. Just narrate what's here. Make sure to rearrange the sensations in chronological order in the description. Do not preface your answer; just dive right in to the narration. Also bear in mind that your senses will deceive you, so be prepared to correct yourself.",
    narrate,
  );

  constructor(
    readonly connection: SocketConnection,
    readonly subscriptions: Subscription[],
  ) {
    logger.info("Establishing memory");
    const startTime = Date.now();

    establishMemory().then(() => {
      const endTime = Date.now();
      logger.info(
        `Memory established in ${endTime - startTime} ms`,
      );

      // Subscribe to errors from the logger
      const errorSubscription = errorSubject.subscribe((sensation) => {
        this.feel(sensation);
      });

      // Add this subscription to the list so it can be managed properly
      this.subscriptions.push(errorSubscription);

      latestSituation().then((situation) => {
        logger.info({ situation }, "Received latest situation");
        this.latestSituation$.next({
          when: new Date(situation.when),
          content: {
            explanation: situation.content,
            content: situation.content,
          },
        });
      });
      handleVision(this);
      handleGeolocations(this);
      handleEchoes(this);
      handleIncomingTexts(this);
      setupHeartbeat(this);

      this.subscriptions.push(this.latestSituation$.subscribe((sensation) => {
        logger.debug({ sensation }, "Received latest situation");
        memorize({
          metadata: {
            label: "Situation",
          },
          data: {
            when: sensation.when.toISOString(),
            content: sensation.content.content,
          },
        });
      }));

      this.subscriptions.push(this.speech$.subscribe((thought) => {
        logger.debug({ thought }, "Received thought");
        this.feel({
          when: new Date(),
          content: {
            explanation: `Here's the conversation I'm having: ${
              this.voice.conversation.map((msg) =>
                `${
                  msg.role === "assistant" ? "Pete Daringsby" : "interlocutor"
                }: ${msg.content}`
              )
                .join("\n")
            }`,
            content: JSON.stringify(this.voice.conversation),
          },
        });
      }));
    }).catch((error) => {
      logger.error({ error }, "Error establishing memory");
    });
  }

  feel(sensation: Sensation<unknown>) {
    logger.debug({ sensation }, "Feeling sensation");
    this.timeline.push(sensation);
    this.timeline.sort((a, b) => a.when.getTime() - b.when.getTime());
    this.integration.feel(sensation);
    this.voice.feel(sensation);
    memorize({
      metadata: {
        label: "Sensation",
      },
      data: {
        when: sensation.when.toISOString(),
        explanation: sensation.content.explanation,
        // content: sensation.content.content, // This might not fit into the graph
      },
    });
  }

  async spin() {
    // Start both the voice and integration processing independently
    this.processVoice();
    this.processIntegration();
  }

  async processVoice() {
    while (this.spinning) {
      try {
        const nextThought = await this.voice.consult().toPromise();
        if (nextThought) {
          this.speech$.next(nextThought);
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
      await new Promise((resolve) => setTimeout(resolve, 500));
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

Session.startIntrospection();
