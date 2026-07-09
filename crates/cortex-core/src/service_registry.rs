//! Type-map service registry for dependency injection.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of services keyed by concrete type.
#[derive(Default)]
pub struct ServiceRegistry {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a service instance. Returns the previous value if one existed.
    pub fn register<S: Any + Send + Sync>(&mut self, service: S) -> Option<Arc<S>> {
        let previous = self.services.insert(TypeId::of::<S>(), Arc::new(service));
        previous.and_then(|arc| arc.downcast::<S>().ok())
    }

    /// Register an already-shared service.
    pub fn register_arc<S: Any + Send + Sync>(&mut self, service: Arc<S>) -> Option<Arc<S>> {
        let previous = self
            .services
            .insert(TypeId::of::<S>(), service as Arc<dyn Any + Send + Sync>);
        previous.and_then(|arc| arc.downcast::<S>().ok())
    }

    /// Get a shared reference to a registered service.
    pub fn get<S: Any + Send + Sync>(&self) -> Option<Arc<S>> {
        self.services
            .get(&TypeId::of::<S>())
            .and_then(|arc| arc.clone().downcast::<S>().ok())
    }

    /// Remove a service. Returns true if one was present.
    pub fn deregister<S: Any + Send + Sync>(&mut self) -> bool {
        self.services.remove(&TypeId::of::<S>()).is_some()
    }

    /// Returns true if a service of type `S` is registered.
    pub fn exists<S: Any + Send + Sync>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<S>())
    }

    /// Number of registered services.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Returns true if no services are registered.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// Type ids of registered services.
    pub fn type_ids(&self) -> Vec<TypeId> {
        self.services.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut reg = ServiceRegistry::new();
        reg.register(42u32);
        assert_eq!(*reg.get::<u32>().unwrap(), 42);
        assert!(reg.exists::<u32>());
        assert!(!reg.exists::<u64>());
        assert!(reg.deregister::<u32>());
        assert!(reg.get::<u32>().is_none());
    }
}
