export enum MessageType {
  Emote = "emote",
  Think = "think",
  Say = "say", // Spoken audio from the bot to be queued for playing on the client
  Echo = "echo", // Acknowledgement that audio has been completely spoken aloud
  Text = "text", // A text message either to or from the bot
  See = "see", // A transmission of the bot's eye (client webcam) to the server
  Geolocate = "geolocate", // A transmission of the client geolocation to the server
  Hear = "hear",
  Heard = "heard",
  Sense = "sense",
}
