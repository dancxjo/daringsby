# Psyche Guidelines
- Every psyche has its own `EventBus`.
- Poll once more after streams finish.
- Sensor trait only; add sensors in binary crates.
- Use Foundation for any dashboard styling; avoid Bootstrap.
- Display queue lengths and timing progress on the scheduler dashboard.
- Prefer `Heart::beat` for background loops instead of timer sleeps.
- Log processor errors instead of dropping them.
- `Heart` implements `Sensor`; use `feel` and `experience` in place of
  `push` and `tick`.
