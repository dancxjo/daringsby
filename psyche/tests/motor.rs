use psyche::{Motor, NoopMotor};

#[tokio::test]
async fn noop_motor_executes() {
    let motor = NoopMotor;
    motor.say("hi").await;
    motor.set_emotion("😊").await;
    motor.take_photo().await;
    motor.focus_on("Travis").await;
}
