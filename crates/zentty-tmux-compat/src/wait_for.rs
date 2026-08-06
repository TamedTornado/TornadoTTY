use crate::ParsedArguments;
use std::collections::BTreeSet;
use std::time::Duration;

pub const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
pub const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq)]
pub enum WaitForAction {
    Signal(String),
    Wait { name: String, timeout: Duration },
}

impl WaitForAction {
    pub const MAX_NAME_BYTES: usize = 128;

    /// Parses the source-compatible signal and wait forms.
    ///
    /// # Errors
    ///
    /// Rejects missing, empty, oversized, control-character, non-finite, and
    /// negative timeout values.
    pub fn parse(arguments: &[String]) -> Result<Self, &'static str> {
        let parsed = ParsedArguments::parse(
            arguments,
            &["--timeout".to_owned()],
            &["-S".to_owned(), "--signal".to_owned()],
        );
        let name = parsed
            .positionals()
            .first()
            .ok_or("wait-for requires a name")?;
        if name.is_empty()
            || name.len() > Self::MAX_NAME_BYTES
            || name.chars().any(char::is_control)
        {
            return Err("wait-for name is invalid or exceeds 128 bytes");
        }
        if parsed.has_flag("-S") || parsed.has_flag("--signal") {
            return Ok(Self::Signal(name.clone()));
        }
        let timeout = parsed.value("--timeout").map_or_else(
            || Ok(DEFAULT_WAIT_TIMEOUT),
            |value| {
                let seconds = value
                    .parse::<f64>()
                    .map_err(|_| "wait-for timeout must be a non-negative finite number")?;
                Duration::try_from_secs_f64(seconds)
                    .map_err(|_| "wait-for timeout must be a non-negative finite duration")
            },
        )?;
        Ok(Self::Wait {
            name: name.clone(),
            timeout,
        })
    }
}

#[derive(Debug, Default)]
pub struct WaitForSignals {
    pending: BTreeSet<String>,
}

impl WaitForSignals {
    pub const MAX_PENDING: usize = 256;

    /// Records one pending named signal. Repeated signals collapse.
    ///
    /// # Errors
    ///
    /// Rejects a new distinct name once the instance-scoped bound is reached.
    pub fn signal(&mut self, name: String) -> Result<(), &'static str> {
        if !self.pending.contains(&name) && self.pending.len() >= Self::MAX_PENDING {
            return Err("wait-for pending signal capacity reached");
        }
        self.pending.insert(name);
        Ok(())
    }

    #[must_use]
    pub fn consume(&mut self, name: &str) -> bool {
        self.pending.remove(name)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_WAIT_TIMEOUT, WaitForAction, WaitForSignals};
    use std::time::Duration;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_signal_default_wait_and_explicit_timeout() {
        assert_eq!(
            WaitForAction::parse(&args(&["-S", "agent-ready"])),
            Ok(WaitForAction::Signal("agent-ready".to_owned()))
        );
        assert_eq!(
            WaitForAction::parse(&args(&["agent-ready"])),
            Ok(WaitForAction::Wait {
                name: "agent-ready".to_owned(),
                timeout: DEFAULT_WAIT_TIMEOUT,
            })
        );
        assert_eq!(
            WaitForAction::parse(&args(&["--timeout", "0.125", "agent-ready"])),
            Ok(WaitForAction::Wait {
                name: "agent-ready".to_owned(),
                timeout: Duration::from_millis(125),
            })
        );
    }

    #[test]
    fn rejects_missing_unsafe_or_invalid_inputs() {
        for invalid in [
            args(&[]),
            args(&["line\nbreak"]),
            args(&[&"x".repeat(WaitForAction::MAX_NAME_BYTES + 1)]),
            args(&["--timeout", "-1", "ready"]),
            args(&["--timeout", "NaN", "ready"]),
            args(&["--timeout", "1e300", "ready"]),
            args(&["--timeout", "infinite", "ready"]),
        ] {
            assert!(WaitForAction::parse(&invalid).is_err(), "{invalid:?}");
        }
        assert!(
            WaitForAction::parse(&args(&[&"x".repeat(WaitForAction::MAX_NAME_BYTES),])).is_ok()
        );
    }

    #[test]
    fn signals_collapse_and_are_consumed_exactly_once() {
        let mut signals = WaitForSignals::default();
        assert!(signals.is_empty());
        signals.signal("ready".to_owned()).unwrap();
        assert!(!signals.is_empty());
        signals.signal("ready".to_owned()).unwrap();
        assert_eq!(signals.len(), 1);
        assert!(signals.consume("ready"));
        assert!(signals.is_empty());
        assert!(!signals.consume("ready"));
    }

    #[test]
    fn pending_names_are_independent_and_bounded() {
        let mut signals = WaitForSignals::default();
        for index in 0..WaitForSignals::MAX_PENDING {
            signals.signal(format!("signal-{index}")).unwrap();
        }
        assert_eq!(signals.len(), WaitForSignals::MAX_PENDING);
        assert!(signals.signal("overflow".to_owned()).is_err());
        signals.signal("signal-0".to_owned()).unwrap();
        assert!(signals.consume("signal-128"));
        signals.signal("replacement".to_owned()).unwrap();
    }
}
