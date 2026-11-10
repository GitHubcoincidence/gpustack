//! Aggregate readiness for the complete Worker lifecycle.

use std::sync::atomic::{AtomicU8, Ordering};

const ALL_GATES: u8 = (1 << 7) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReadinessGate {
    Resources = 1 << 0,
    Docker = 1 << 1,
    WorkerApi = 1 << 2,
    Metrics = 1 << 3,
    Identity = 1 << 4,
    FullSync = 1 << 5,
    Runtime = 1 << 6,
}

impl ReadinessGate {
    const ALL: [Self; 7] = [
        Self::Resources,
        Self::Docker,
        Self::WorkerApi,
        Self::Metrics,
        Self::Identity,
        Self::FullSync,
        Self::Runtime,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Resources => "resources",
            Self::Docker => "docker",
            Self::WorkerApi => "worker_api",
            Self::Metrics => "metrics",
            Self::Identity => "identity",
            Self::FullSync => "full_sync",
            Self::Runtime => "runtime",
        }
    }

    const fn bit(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessSnapshot {
    pub ready: bool,
    pub pending: Vec<&'static str>,
}

#[derive(Debug, Default)]
pub struct WorkerReadiness {
    gates: AtomicU8,
}

impl WorkerReadiness {
    pub fn set(&self, gate: ReadinessGate, ready: bool) {
        if ready {
            self.gates.fetch_or(gate.bit(), Ordering::AcqRel);
        } else {
            self.gates.fetch_and(!gate.bit(), Ordering::AcqRel);
        }
    }

    pub fn reset(&self) {
        self.gates.store(0, Ordering::Release);
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.gates.load(Ordering::Acquire) == ALL_GATES
    }

    #[must_use]
    pub fn snapshot(&self) -> ReadinessSnapshot {
        let gates = self.gates.load(Ordering::Acquire);
        ReadinessSnapshot {
            ready: gates == ALL_GATES,
            pending: ReadinessGate::ALL
                .into_iter()
                .filter(|gate| gates & gate.bit() == 0)
                .map(ReadinessGate::name)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gate_is_required_and_any_regression_clears_readiness() {
        let readiness = WorkerReadiness::default();

        for gate in ReadinessGate::ALL {
            assert!(!readiness.is_ready());
            readiness.set(gate, true);
        }
        assert!(readiness.is_ready());
        assert!(readiness.snapshot().pending.is_empty());

        readiness.set(ReadinessGate::FullSync, false);
        assert_eq!(
            readiness.snapshot(),
            ReadinessSnapshot {
                ready: false,
                pending: vec!["full_sync"],
            }
        );

        readiness.reset();
        assert_eq!(readiness.snapshot().pending.len(), 7);
    }
}
