import { logger } from "../core/logger.ts";
import { catchError, Observable, of, tap } from "npm:rxjs";
import { Faculty, Sensation } from "../core/interfaces.ts";
import Handlebars from "npm:handlebars";

export class Genie<I> implements Faculty<I, string> {
  protected sensations: Sensation<I>[] = [];

  constructor(
    protected name: string,
    protected description: string,
    protected instruction: string,
    protected narrate: (prompt: string) => Observable<string>,
  ) {
    logger.info(`Initializing Genie: ${name}`);
    this.feel({
      when: new Date(),
      content: {
        explanation: `Initialized ${name}`,
        content: `Poke the quick to start the ${name}`,
      },
    });
  }

  feel(sensation: Sensation<I>) {
    logger.info(`${this.name}: Feeling sensation`);
    this.sensations.push(sensation);
  }

  protected generatePrompt<I>(input: I): string {
    const templateString =
      `You are the ${this.name}. ${this.description} ${this.instruction}`;
    const compiledTemplate = Handlebars.compile(templateString);
    const prompt = compiledTemplate(input);
    return prompt;
  }

  consult(): Observable<string> {
    logger.info(`${this.name}: Consulting`);
    const input = {
      name: this.name,
      description: this.description,
      instruction: this.instruction,
      sensations: this.sensations,
    };

    if (!this.sensations.length) {
      logger.error(`${this.name}: No sensations to narrate`);
      return of("");
    }
    this.sensations = [];
    logger.debug({ input }, `${this.name}: Input for template`);
    const prompt = this.generatePrompt(input);

    logger.debug({ prompt }, `${this.name}: Prompt generated`);

    const narration$ = this.narrate(prompt).pipe(
      tap((response) =>
        logger.debug(`${this.name}: LLM response received`, {
          response,
        })
      ),
      catchError((err) => {
        logger.error(`${this.name}: Error invoking LLM`, err);
        return of("");
      }),
    );

    return narration$;
  }
}
