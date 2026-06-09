//! Projects domain events onto the persisted session timeline.

use std::sync::Arc;

use chrono::Utc;

use agentx_domain::{DomainEvent, PersistedEvent, SessionRepository, StoreError};

/// Writes appended session events to the [`SessionRepository`].
///
/// The composition root subscribes this to the event bus and hands each
/// [`DomainEvent`] to [`project`](Self::project). Only
/// [`DomainEvent::SessionAppended`] becomes a persisted row — stamped with the
/// time it was projected — so persistence stays a single, decoupled concern
/// rather than something every use-case must remember to do.
pub struct PersistenceProjector {
    repository: Arc<dyn SessionRepository>,
}

impl PersistenceProjector {
    pub fn new(repository: Arc<dyn SessionRepository>) -> Self {
        Self { repository }
    }

    /// Persist one event if it belongs on a session timeline; ignore the rest.
    pub async fn project(&self, event: &DomainEvent) -> Result<(), StoreError> {
        let DomainEvent::SessionAppended { session, event } = event else {
            return Ok(());
        };
        self.repository
            .append(session, PersistedEvent::new(event.clone(), Utc::now()))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeSessionRepository;
    use agentx_domain::{AgentId, AgentStatus, ContentBlock, SessionEvent, SessionId};

    #[tokio::test]
    async fn session_appended_events_are_persisted() {
        let repository = Arc::new(FakeSessionRepository::new());
        let projector = PersistenceProjector::new(repository.clone());
        let session = SessionId::from("s1");
        let event = SessionEvent::AgentMessage {
            content: vec![ContentBlock::text("hi")],
        };

        projector
            .project(&DomainEvent::SessionAppended {
                session: session.clone(),
                event: event.clone(),
            })
            .await
            .unwrap();

        let stored = repository.load(&session).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].event, event);
    }

    #[tokio::test]
    async fn non_timeline_events_are_ignored() {
        let repository = Arc::new(FakeSessionRepository::new());
        let projector = PersistenceProjector::new(repository.clone());

        projector
            .project(&DomainEvent::AgentStatusChanged {
                agent: AgentId::from("claude"),
                status: AgentStatus::Ready,
            })
            .await
            .unwrap();
        projector.project(&DomainEvent::ConfigChanged).await.unwrap();

        assert!(repository.list().await.unwrap().is_empty());
    }
}
