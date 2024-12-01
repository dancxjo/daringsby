import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { logger } from "../logger.ts";

interface TextInputProps {
  onChange?: (_newValue: string) => void;
}

export default function TextInput(props: TextInputProps) {
  const input = useSignal("");
  const actual = useSignal("");

  useEffect(() => {
    logger.debug("TextInput mounted");
    if (props.onChange) {
      logger.debug("TextInput onChange");
      props.onChange(actual.value);
    }
  }, [actual.value]);

  const handleSubmit = (e: Event) => {
    e.preventDefault();
    logger.debug("TextInput handleSubmit");
    actual.value = input.value;
    input.value = "";
  };

  return (
    <form
      onSubmit={handleSubmit}
      class="input-group mb-3"
    >
      <input
        class="user-message-to-ai form-control"
        type="text"
        value={input.value}
        onInput={(e) => {
          input.value = (e.target as HTMLInputElement).value;
        }}
        autoFocus
      />
      <button
        class="btn btn-primary"
        type="submit"
      >
        Send
      </button>
    </form>
  );
}
