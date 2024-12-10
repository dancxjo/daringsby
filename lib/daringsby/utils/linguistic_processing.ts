import { Observable, ReplaySubject } from "npm:rxjs";
import { ChatResponse, Message, Ollama, Options } from "npm:ollama";

export interface Streamed<T> {
  readonly inChunks$: Observable<string>; // Stream of raw data chunks
  readonly result: Promise<T>; // Processed final result
}

export class ChatStream<T = string> implements Streamed<T> {
  protected completeMessage: string = "";
  protected chunks = new ReplaySubject<string>();
  readonly inChunks$: Observable<string> = this.chunks.asObservable();
  readonly result: Promise<T>;

  constructor(
    protected ollama: Ollama,
    protected messages: Message[] = [],
    protected model: string,
    protected options: Partial<Options> = {},
    protected transform: (message: Message) => T = (message) =>
      message.content as unknown as T,
  ) {
    this.result = this.initializeStream();
  }

  private async initializeStream(): Promise<T> {
    try {
      const stream = await this.ollama.chat({
        messages: this.messages,
        model: this.model,
        stream: true,
        options: this.options,
      });

      for await (const chunk of stream) {
        this.chunks.next(chunk.message.content);
        this.completeMessage += chunk.message.content;
      }

      this.chunks.complete();
      return this.transform({
        role: "assistant",
        content: this.completeMessage,
      });
    } catch (error) {
      this.chunks.error(error);
      throw error;
    }
  }

  toString(): string {
    return this.completeMessage;
  }

  protected handleAbort(): void {
    this.ollama.abort();
    this.chunks.error(new Error("Stream aborted"));
  }
}
