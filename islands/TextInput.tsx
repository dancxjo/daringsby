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

    const handleSubmit = () => {
        logger.debug("TextInput handleSubmit");
        actual.value = input.value;
        input.value = "";
    };

    return (
        <form
            onSubmit={(e) => e.preventDefault()}
            class="flex items-center space-x-4"
        >
            <input
                class="flex-1 border border-gray-300 rounded-lg p-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
                type="text"
                value={input.value}
                onInput={(e) => {
                    input.value = (e.target as HTMLInputElement).value;
                }}
                autoFocus
            />
            <button
                class="bg-blue-500 text-white rounded-lg px-4 py-2 hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
                type="default"
                onClick={handleSubmit}
            >
                Send
            </button>
        </form>
    );
}
