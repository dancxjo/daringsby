import { Signal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";

interface SWProps {
  words: Signal<string>;
}

export default function SpokenWords({ words }: SWProps) {
  const forwardRef = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (forwardRef.current) {
      forwardRef.current.scrollTop = forwardRef.current.scrollHeight;
    }
  }, [words.value]);

  return (
    <textarea ref={forwardRef} disabled class="spoken-words">
      {words.value}
    </textarea>
  );
}
