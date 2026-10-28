//! In-process readiness and healthy route publication for Managed workloads.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;

use super::{MANAGED_PORT_END, MANAGED_PORT_START};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRoute {
    pub instance_id: i64,
    pub port: u16,
    pub served_model_name: String,
}

#[derive(Debug, Default)]
pub struct ManagedRuntimeRegistry {
    ready: AtomicBool,
    routes: RwLock<BTreeMap<i64, ManagedRoute>>,
}

impl ManagedRuntimeRegistry {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    pub fn publish(
        &self,
        instance_id: i64,
        port: u16,
        served_model_name: String,
    ) -> Result<(), ManagedRegistryError> {
        if instance_id <= 0
            || !(MANAGED_PORT_START..=MANAGED_PORT_END).contains(&port)
            || served_model_name.trim().is_empty()
        {
            return Err(ManagedRegistryError::InvalidRoute);
        }
        self.write_routes()?.insert(
            instance_id,
            ManagedRoute {
                instance_id,
                port,
                served_model_name,
            },
        );
        Ok(())
    }

    pub fn remove(&self, instance_id: i64) -> Result<(), ManagedRegistryError> {
        self.write_routes()?.remove(&instance_id);
        Ok(())
    }

    pub fn route(&self, instance_id: i64) -> Result<Option<ManagedRoute>, ManagedRegistryError> {
        Ok(self.read_routes()?.get(&instance_id).cloned())
    }

    fn read_routes(
        &self,
    ) -> Result<RwLockReadGuard<'_, BTreeMap<i64, ManagedRoute>>, ManagedRegistryError> {
        self.routes
            .read()
            .map_err(|_| ManagedRegistryError::LockPoisoned)
    }

    fn write_routes(
        &self,
    ) -> Result<RwLockWriteGuard<'_, BTreeMap<i64, ManagedRoute>>, ManagedRegistryError> {
        self.routes
            .write()
            .map_err(|_| ManagedRegistryError::LockPoisoned)
    }
}

#[derive(Debug, Error)]
pub enum ManagedRegistryError {
    #[error("Managed route is outside the R1 runtime contract")]
    InvalidRoute,
    #[error("Managed route registry lock failed")]
    LockPoisoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_only_valid_managed_routes_and_tracks_readiness() {
        let registry = ManagedRuntimeRegistry::default();
        assert!(!registry.is_ready());
        assert!(registry.route(301).expect("route lookup").is_none());

        assert!(registry.publish(0, 40_000, "qwen".to_string()).is_err());
        assert!(registry.publish(301, 39_999, "qwen".to_string()).is_err());
        registry
            .publish(301, 40_000, "qwen".to_string())
            .expect("valid route");
        assert_eq!(
            registry.route(301).expect("route lookup"),
            Some(ManagedRoute {
                instance_id: 301,
                port: 40_000,
                served_model_name: "qwen".to_string(),
            })
        );

        registry.set_ready(true);
        assert!(registry.is_ready());
        registry.remove(301).expect("remove route");
        assert!(registry.route(301).expect("route lookup").is_none());
    }
}
