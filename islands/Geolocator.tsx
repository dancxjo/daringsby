import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";

interface GeolocatorProps {
    onChange?: (
        location: { longitude: number; latitude: number },
    ) => void;
}

export default function Geolocator(props: GeolocatorProps) {
    const location = useSignal({ longitude: 0, latitude: 0 });
    useEffect(() => {
        navigator.geolocation.getCurrentPosition((position) => {
            const coords = {
                longitude: position.coords.longitude,
                latitude: position.coords.latitude,
            };
            if (!location.value && props.onChange) {
                props.onChange(coords);
            }
            location.value = coords;
        });
    }, []);

    useEffect(() => {
        if (props.onChange) {
            props.onChange(location.value);
        }
    }, [location.value]);
    return (
        <div>
            Reporting geolocation
        </div>
    );
}
