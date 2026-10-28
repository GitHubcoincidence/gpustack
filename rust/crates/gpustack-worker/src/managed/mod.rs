//! Managed workload preparation without a container runtime.

mod coordinator;
mod model_file;
mod port;
mod reconciler;
mod runtime;
mod store;

pub use coordinator::{
    ManagedApi, ManagedCoordinator, ManagedOutcome, ManagedResult, ManagedTarget,
};
pub use model_file::{LocalModelResolver, ResolveError, ResolvedModelFile};
pub use port::{
    MANAGED_PORT_END, MANAGED_PORT_START, ManagedPortAllocator, PortAllocationError,
    PortAssignment, PortProbe, TcpBindProbe,
};
pub use reconciler::{
    ManagedClock, ManagedGpuProbe, ManagedRuntimeApi, ManagedRuntimeCoordinator,
    ManagedRuntimeError, ManagedRuntimeOutcome, ManagedRuntimeResult, NvmlManagedGpuProbe,
    SystemManagedClock, run_production_managed_runtime,
};
pub use runtime::{ManagedGpu, ManagedRuntimePlanError, ManagedWorkloadPlanner};
pub use store::{
    FsManagedRuntimeStore, MANAGED_MANIFEST_SCHEMA_VERSION, ManagedRuntimeManifest,
    ManagedRuntimeStore, ManagedStoreError,
};
