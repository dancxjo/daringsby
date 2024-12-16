import { Signal, useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";

interface GeolocatorProps {
  onChange?: (
    location: { longitude: number; latitude: number },
  ) => void;
}

export type Geolocation = {
  longitude: number;
  latitude: number;
};

function ubificate(
  location: Signal<Geolocation>,
  onChange?: (location: Geolocation) => void,
) {
  navigator.geolocation.getCurrentPosition((position) => {
    const coords = {
      longitude: position.coords.longitude,
      latitude: position.coords.latitude,
    };
    if (!location.value && onChange) {
      onChange(coords);
    }
    location.value = coords;
  });
}

export default function Geolocator(props: GeolocatorProps) {
  const location = useSignal({ longitude: 0, latitude: 0 });
  useEffect(() => {
    ubificate(location);
    const id = setInterval(() => {
      ubificate(location);
    }, 60000 * 15);
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
