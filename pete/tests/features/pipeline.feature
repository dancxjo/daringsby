Feature: Pete's conversational pipeline

  Scenario: Basic Greeting Echo with Emoji Feedback
    Given Pete is running with an active interface
    And the front-end displays a default emoji 😐
    And the LLM is mocked to reply "Hello there! 🙂" to "Hello, Pete"
    When the user sends "Hello, Pete"
    Then Pete says "Hello there! 🙂"
    And the front-end emoji becomes 🙂

  Scenario: No Response Without Will
    Given Pete is running with an active interface
    And Will has not authorized a turn
    When the user sends "Hello?"
    Then no speech is produced

  Scenario: Inline Emoji Routing
    Given Pete is running with an active interface
    And the LLM is mocked to reply "I'm excited! 😆" to any input
    When the user sends "Say something"
    Then Pete says "I'm excited! 😆"
    And the TTS system receives "I'm excited!"
    And the front-end emoji becomes 😆

  Scenario: Echo Confirmation
    Given Pete is running with an active interface
    And the LLM is mocked to reply "Echo" to any input
    When the user sends "test"
    And the front-end acknowledges playback of "Echo"
    Then the psyche conversation contains "Echo" from assistant

  Scenario: Prompt Synchronization
    Given Pete is running with an active interface
    And a wit provides context "Weather is nice"
    And the LLM is mocked to reply "ok" to any input
    When the user sends "update"
    Then the voice prompt used contains "Weather is nice"

  Scenario: TTS Dispatch Confirmation
    Given Pete is running with an active interface
    And the LLM is mocked to reply "Hi" to any input
    When the user sends "say hi"
    Then the TTS system receives "Hi"
