import { Signal } from "@preact/signals";

interface ThoughtBubbleProps {
    thought: Signal<string>;
}

export default function ThoughtBubble({ thought }: ThoughtBubbleProps) {
    return <div class="thought-bubble">{thought.value ?? " "}</div>;
}
