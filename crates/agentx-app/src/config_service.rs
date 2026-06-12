//! Configuration read/save use-case.
//!
//! A thin use-case over the [`ConfigStore`] port: it loads the app config and
//! persists edits, publishing [`DomainEvent::ConfigChanged`] so the rest of the
//! app reacts. Agent *process* management (add/remove/restart) stays on the
//! [`AgentRegistry`](agentx_domain::AgentRegistry) port, which the settings panel
//! drives directly — this service owns only the persisted config.

use std::sync::Arc;

use agentx_bus::EventBus;
use agentx_domain::{Config, ConfigStore, DomainEvent, StoreError};

pub struct ConfigService {
    store: Arc<dyn ConfigStore>,
    bus: EventBus,
}

impl ConfigService {
    pub fn new(store: Arc<dyn ConfigStore>, bus: EventBus) -> Self {
        Self { store, bus }
    }

    /// Load the persisted configuration.
    pub async fn load(&self) -> Result<Config, StoreError> {
        self.store.load().await
    }

    /// Persist the configuration and notify the app that it changed.
    pub async fn save(&self, config: Config) -> Result<(), StoreError> {
        self.store.save(&config).await?;
        self.bus.publish(DomainEvent::ConfigChanged);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeConfigStore;

    #[tokio::test]
    async fn save_persists_and_publishes_config_changed() {
        let store = Arc::new(FakeConfigStore::new());
        let bus = EventBus::new();
        let mut rx = bus.subscribe::<DomainEvent>();
        let service = ConfigService::new(store.clone(), bus.clone());

        let mut config = Config::default();
        config.proxy.enabled = true;
        service.save(config).await.unwrap();

        let saved = store.saved();
        assert_eq!(saved.len(), 1);
        assert!(saved[0].proxy.enabled);

        let mut published = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, DomainEvent::ConfigChanged) {
                published = true;
            }
        }
        assert!(published);
    }
}
