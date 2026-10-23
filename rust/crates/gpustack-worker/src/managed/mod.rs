//! Managed workload preparation without a container runtime.

mod coordinator;
mod model_file;
mod port;

pub use coordinator::{
    ManagedApi, ManagedCoordinator, ManagedOutcome, ManagedResult, ManagedTarget,
};
pub use model_file::{LocalModelResolver, ResolveError, ResolvedModelFile};
pub use port::{
    MANAGED_PORT_END, MANAGED_PORT_START, ManagedPortAllocator, PortAllocationError,
    PortAssignment, PortProbe, TcpBindProbe,
};
