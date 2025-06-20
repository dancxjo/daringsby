import { Signal } from "@preact/signals";

interface MienProps {
    mien: Signal<string>;
}

export default function Mien({ mien }: MienProps) {
    return <div class="mien">{mien.value ?? " "}</div>;
}
