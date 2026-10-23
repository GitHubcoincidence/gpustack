//! Managed service-port selection with an injectable bind probe.

use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::ops::RangeInclusive;

use gpustack_client::types::ModelInstancePublic;
use thiserror::Error;

pub const MANAGED_PORT_START: u16 = 40_000;
pub const MANAGED_PORT_END: u16 = 40_063;

pub trait PortProbe: Send + Sync {
    fn is_available(&self, address: SocketAddr) -> io::Result<bool>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TcpBindProbe;

impl PortProbe for TcpBindProbe {
    fn is_available(&self, address: SocketAddr) -> io::Result<bool> {
        match TcpListener::bind(address) {
            Ok(listener) => {
                drop(listener);
                Ok(true)
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => Ok(false),
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortAssignment {
    pub port: u16,
    pub reused: bool,
}

pub struct ManagedPortAllocator<P> {
    range: RangeInclusive<u16>,
    probe: P,
}

impl ManagedPortAllocator<TcpBindProbe> {
    pub fn production() -> Self {
        Self::new(MANAGED_PORT_START..=MANAGED_PORT_END, TcpBindProbe)
            .expect("the fixed Managed port range is valid")
    }
}

impl<P: PortProbe> ManagedPortAllocator<P> {
    pub fn new(range: RangeInclusive<u16>, probe: P) -> Result<Self, PortAllocationError> {
        if range.is_empty() || *range.start() == 0 {
            return Err(PortAllocationError::InvalidRange);
        }
        Ok(Self { range, probe })
    }

    pub fn assign(
        &self,
        instance: &ModelInstancePublic,
        worker_id: i64,
        worker_ip: IpAddr,
        authoritative_instances: &[ModelInstancePublic],
    ) -> Result<PortAssignment, PortAllocationError> {
        let unavailable = occupied_ports(authoritative_instances, worker_id, instance.id);
        if let Some(port) = instance.port {
            if !self.range.contains(&port) {
                return Err(PortAllocationError::ExistingPortOutsideRange);
            }
            if unavailable.contains(&port) {
                return Err(PortAllocationError::ExistingPortConflicts);
            }
            return self.verify_candidate(worker_ip, port, true);
        }

        for port in self.range.clone() {
            if unavailable.contains(&port) {
                continue;
            }
            match self.verify_candidate(worker_ip, port, false) {
                Ok(assignment) => return Ok(assignment),
                Err(PortAllocationError::PortOccupied) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(PortAllocationError::Exhausted)
    }

    fn verify_candidate(
        &self,
        worker_ip: IpAddr,
        port: u16,
        reused: bool,
    ) -> Result<PortAssignment, PortAllocationError> {
        let address = SocketAddr::new(worker_ip, port);
        match self.probe.is_available(address) {
            Ok(true) => Ok(PortAssignment { port, reused }),
            Ok(false) => Err(PortAllocationError::PortOccupied),
            Err(error) => Err(PortAllocationError::Probe { kind: error.kind() }),
        }
    }
}

fn occupied_ports(
    instances: &[ModelInstancePublic],
    worker_id: i64,
    target_id: i64,
) -> BTreeSet<u16> {
    let mut ports = BTreeSet::new();
    for instance in instances
        .iter()
        .filter(|instance| instance.worker_id == Some(worker_id) && instance.id != target_id)
    {
        ports.extend(instance.port);
        ports.extend(instance.ports.iter().flatten().copied());
    }
    ports
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PortAllocationError {
    #[error("Managed service-port range is invalid")]
    InvalidRange,
    #[error("the existing Managed service port is outside the configured range")]
    ExistingPortOutsideRange,
    #[error("the existing Managed service port conflicts with another assigned instance")]
    ExistingPortConflicts,
    #[error("the Managed service port is occupied by an unknown listener")]
    PortOccupied,
    #[error("no Managed service port is available")]
    Exhausted,
    #[error("failed to probe a Managed service port: {kind:?}")]
    Probe { kind: io::ErrorKind },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    use gpustack_client::types::{ModelInstancePublic, Paginated};

    use super::*;

    #[derive(Default)]
    struct FakeProbe {
        occupied: BTreeSet<u16>,
        probed: Mutex<Vec<u16>>,
    }

    impl FakeProbe {
        fn with_occupied(ports: impl IntoIterator<Item = u16>) -> Self {
            Self {
                occupied: ports.into_iter().collect(),
                probed: Mutex::new(Vec::new()),
            }
        }
    }

    impl PortProbe for FakeProbe {
        fn is_available(&self, address: SocketAddr) -> io::Result<bool> {
            self.probed.lock().expect("probe lock").push(address.port());
            Ok(!self.occupied.contains(&address.port()))
        }
    }

    fn fixture_instance() -> ModelInstancePublic {
        serde_json::from_slice::<Paginated<ModelInstancePublic>>(include_bytes!(
            "../../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json"
        ))
        .expect("instance fixture")
        .items
        .remove(0)
    }

    #[test]
    fn excludes_authoritative_and_probe_occupied_ports() {
        let target = fixture_instance();
        let mut other = fixture_instance();
        other.id = 302;
        other.port = Some(40_000);
        other.ports = Some(vec![40_000, 40_001]);
        let allocator =
            ManagedPortAllocator::new(40_000..=40_003, FakeProbe::with_occupied([40_002]))
                .expect("allocator");

        let assignment = allocator
            .assign(&target, 101, "127.0.0.1".parse().expect("IP"), &[other])
            .expect("available port");

        assert_eq!(assignment.port, 40_003);
        assert!(!assignment.reused);
    }

    #[test]
    fn reuses_a_valid_existing_port_only_after_probing_it() {
        let mut target = fixture_instance();
        target.port = Some(40_002);
        target.ports = Some(vec![40_002]);
        let allocator =
            ManagedPortAllocator::new(40_000..=40_003, FakeProbe::default()).expect("allocator");

        let assignment = allocator
            .assign(&target, 101, "127.0.0.1".parse().expect("IP"), &[])
            .expect("reused port");

        assert_eq!(assignment.port, 40_002);
        assert!(assignment.reused);
        assert_eq!(
            allocator
                .probe
                .probed
                .lock()
                .expect("probe lock")
                .as_slice(),
            [40_002]
        );
    }

    #[test]
    fn refuses_a_conflicting_existing_port() {
        let mut target = fixture_instance();
        target.port = Some(40_001);
        let mut other = fixture_instance();
        other.id = 302;
        other.port = Some(40_001);
        let allocator =
            ManagedPortAllocator::new(40_000..=40_003, FakeProbe::default()).expect("allocator");

        assert_eq!(
            allocator
                .assign(&target, 101, "127.0.0.1".parse().expect("IP"), &[other],)
                .expect_err("port conflict"),
            PortAllocationError::ExistingPortConflicts
        );
    }

    #[test]
    fn reports_exhaustion_when_every_candidate_is_occupied() {
        let target = fixture_instance();
        let allocator =
            ManagedPortAllocator::new(40_000..=40_001, FakeProbe::with_occupied([40_000, 40_001]))
                .expect("allocator");

        assert_eq!(
            allocator
                .assign(&target, 101, "127.0.0.1".parse().expect("IP"), &[])
                .expect_err("exhausted port range"),
            PortAllocationError::Exhausted
        );
    }
}
