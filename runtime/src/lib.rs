//! Runtime orchestrator for Pete Daringsby.

/// Determine the tick rate in seconds from CLI or `TICK_RATE` env.
/// Falls back to `1.0` if unset or invalid.
pub fn tick_rate(cli: Option<f32>) -> f32 {
    cli.or_else(|| std::env::var("TICK_RATE").ok()?.parse::<f32>().ok())
        .unwrap_or(1.0)
}

pub mod server;
pub mod logger;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn cli_overrides_env() {
        env::set_var("TICK_RATE", "2.0");
        assert_eq!(tick_rate(Some(0.5)), 0.5);
    }

    #[test]
    fn env_used_when_cli_none() {
        env::set_var("TICK_RATE", "3.5");
        assert_eq!(tick_rate(None), 3.5);
        env::remove_var("TICK_RATE");
    }

    #[test]
    fn default_when_missing() {
        env::remove_var("TICK_RATE");
        assert_eq!(tick_rate(None), 1.0);
    }
}
