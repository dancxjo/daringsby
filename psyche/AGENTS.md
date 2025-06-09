# Psyche Guidelines
- Every psyche has its own `EventBus`.
- Poll once more after streams finish.
- Keep sensors modular with examples.
- Use Foundation for any dashboard styling; avoid Bootstrap.
- Display queue lengths and timing progress on the scheduler dashboard.
- Prefer `Heart::run_serial` for background loops instead of timer sleeps.
- Log processor errors instead of dropping them.
