export enum MessageType {
    Emote,
    Think,
    Say, // Spoken audio from the bot to be queued for playing on the client
    Echo, // Acknowledgement that audio has been completely spoken aloud
    Text, // A text message either to or from the bot
    See, // A transmission of the bot's eye (client webcam) to the server
    Geolocate, // A transmission of the client geolocation to the server
    Hear,
}
