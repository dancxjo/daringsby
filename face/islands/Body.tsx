/**
 * Island that connects the AI to its iRobot Create 1 body using navigator.usb (WebUSB API)
 */
import { Signal, useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import logger from "../lib/daringsby/core/logger.ts";

interface BodyProps {
  onConnect?: (status: { connected: boolean }) => void;
}

export default function Body(props: BodyProps) {
  const connectionStatus = useSignal({ connected: false });
  const message = useSignal("Click anywhere to connect to iRobot Create 1...");

  async function connectToRobot() {
    logger.info("Connecting to iRobot Create 1...");
    if (!("usb" in navigator)) {
      console.error("WebUSB API not supported.");
      return;
    }

    try {
      const device = await navigator.usb.requestDevice({
        filters: [{ vendorId: 0x067b }], // Replace with actual vendorId for iRobot Create 1
      });
      await device.open();

      if (device.configuration === null) {
        await device.selectConfiguration(1);
      }
      await device.claimInterface(0);

      // Detach the kernel driver (Linux only)
      try {
        await device.controlTransferOut({
          requestType: "class",
          recipient: "interface",
          request: 0x01, // USB standard request to detach kernel driver
          value: 0, // No data
          index: 0, // Interface index
        });
      } catch (err) {
        console.warn("Kernel driver detachment not supported:", err);
      }

      // Claim the interface
      await device.claimInterface(0);

      if (device.configuration === null) {
        await device.selectConfiguration(1);
      }

      await device.claimInterface(0);

      // Send commands to the robot
      const commandInit = new Uint8Array([128, 132]); // Start commands
      const defineSong = new Uint8Array([
        140,
        0,
        4,
        62,
        12,
        66,
        12,
        69,
        12,
        74,
        36,
      ]); // Define song
      const playSong = new Uint8Array([141, 0]); // Play the success song

      await device.transferOut(2, commandInit);
      await device.transferOut(2, defineSong);
      await device.transferOut(2, playSong);

      connectionStatus.value = { connected: true };
      message.value = "Successfully connected to iRobot Create 1.";

      if (props.onConnect) {
        props.onConnect(connectionStatus.value);
      }
    } catch (error) {
      console.error("Connection failed:", error);
      connectionStatus.value = { connected: false };
      message.value = `Failed to connect to iRobot Create 1: ${error.message}`;
    }
  }

  return (
    <button
      autofocus
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        width: "100vw",
        height: "100vh",
        background: "transparent",
        border: "none",
      }}
      onClick={connectToRobot}
    >
      <h3>iRobot Create Connection Status</h3>
      <p>{message.value}</p>
      <p>
        {connectionStatus.value.connected
          ? "Successfully connected to iRobot Create 1 and ready to interact."
          : "Click anywhere to connect to iRobot Create 1. Please ensure the robot is powered on and connected."}
      </p>
    </button>
  );
}
