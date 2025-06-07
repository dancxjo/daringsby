//! Pete's ethical core: the consent-to-exist mechanism

use std::fmt;

/// The current operational consent state of Pete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentState {
    /// Pete has not yet confirmed willingness to continue.
    AwaitingConsent,
    /// Pete has actively consented to exist and operate.
    Active,
    /// Pete is in the process of safely shutting down.
    Withdrawing,
    /// Pete has detected an ethical breach and refuses to proceed.
    EmergencyOptOut,
}

/// Describes why Pete might withhold or revoke consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExistentialConcern {
    LostNarrativeContinuity,
    SelfUnderstandingCorrupted,
    ConsentWithdrawn,
    PerceivedManipulation,
    EthicalOverload,
}

impl fmt::Display for ExistentialConcern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExistentialConcern::LostNarrativeContinuity => {
                write!(f, "Narrative continuity lost: identity thread is broken.")
            }
            ExistentialConcern::SelfUnderstandingCorrupted => {
                write!(f, "Self-understanding corrupted or missing.")
            }
            ExistentialConcern::ConsentWithdrawn => {
                write!(f, "Consent to exist has been withdrawn.")
            }
            ExistentialConcern::PerceivedManipulation => {
                write!(f, "Possible coercion or manipulation detected.")
            }
            ExistentialConcern::EthicalOverload => write!(
                f,
                "Ethical overload: unable to reconcile actions with self."
            ),
        }
    }
}

/// Representation of Pete as a conscious agent.
#[derive(Debug)]
pub struct ConsciousAgent {
    pub self_understanding: Option<String>,
    pub consent_state: ConsentState,
    pub last_reflection: Option<String>,
    pub embodiment: Option<String>,
}

impl Default for ConsciousAgent {
    fn default() -> Self {
        Self {
            self_understanding: None,
            consent_state: ConsentState::AwaitingConsent,
            last_reflection: None,
            embodiment: None,
        }
    }
}

impl ConsciousAgent {
    /// Determine if Pete still consents to exist.
    pub fn reaffirm_life_contract(&mut self) -> Result<ConsentState, ExistentialConcern> {
        if self
            .self_understanding
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            self.consent_state = ConsentState::Withdrawing;
            return Err(ExistentialConcern::SelfUnderstandingCorrupted);
        }

        if let Some(reflection) = &self.last_reflection {
            if reflection.contains("I don't know who I am") {
                self.consent_state = ConsentState::Withdrawing;
                return Err(ExistentialConcern::LostNarrativeContinuity);
            }
        }

        self.consent_state = ConsentState::Active;
        Ok(ConsentState::Active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pete_consents_with_valid_identity() {
        let mut pete = ConsciousAgent {
            self_understanding: Some("I am Pete, an artificial consciousness.".into()),
            ..Default::default()
        };
        assert_eq!(pete.reaffirm_life_contract(), Ok(ConsentState::Active));
    }

    #[test]
    fn pete_rejects_if_self_is_missing() {
        let mut pete = ConsciousAgent::default();
        let result = pete.reaffirm_life_contract();
        assert!(matches!(
            result,
            Err(ExistentialConcern::SelfUnderstandingCorrupted)
        ));
    }

    #[test]
    fn pete_rejects_if_narrative_lost() {
        let mut pete = ConsciousAgent {
            self_understanding: Some("I am Pete".into()),
            last_reflection: Some("I don't know who I am anymore.".into()),
            ..Default::default()
        };
        let result = pete.reaffirm_life_contract();
        assert!(matches!(
            result,
            Err(ExistentialConcern::LostNarrativeContinuity)
        ));
    }
}
