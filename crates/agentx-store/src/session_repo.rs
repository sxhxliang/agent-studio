//! Filesystem implementation of [`SessionRepository`].

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use async_trait::async_trait;

use agentx_domain::{PersistedEvent, SessionId, SessionRepository, StoreError};

use crate::{io_err, serde_err};

/// Stores each session's timeline as a JSON Lines file — one [`PersistedEvent`]
/// per line — at `{root}/{session_id}.jsonl`.
///
/// Append-only by design: the ACP adapter accumulates streamed chunks into
/// complete [`agentx_domain::SessionEvent`]s before they reach this repository,
/// so there is no buffering or batching to do here.
///
/// I/O is blocking `std::fs`: session files are tiny, and using plain blocking
/// calls lets these async methods be awaited from any executor — notably GPUI's,
/// which is not a Tokio runtime. Callers that must not block (e.g. the streaming
/// persistence loop) run it off the UI thread; tests and one-off reads await it
/// directly.
pub struct FsSessionRepository {
    root: PathBuf,
}

impl FsSessionRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn file(&self, session: &SessionId) -> PathBuf {
        self.root.join(format!("{}.jsonl", session.as_str()))
    }
}

#[async_trait]
impl SessionRepository for FsSessionRepository {
    async fn append(&self, session: &SessionId, event: PersistedEvent) -> Result<(), StoreError> {
        fs::create_dir_all(&self.root).map_err(io_err)?;
        let mut line = serde_json::to_string(&event).map_err(serde_err)?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.file(session))
            .map_err(io_err)?;
        file.write_all(line.as_bytes()).map_err(io_err)?;
        Ok(())
    }

    async fn load(&self, session: &SessionId) -> Result<Vec<PersistedEvent>, StoreError> {
        let data = fs::read_to_string(self.file(session)).map_err(io_err)?;
        let mut events = Vec::new();
        for line in data.lines() {
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(line).map_err(serde_err)?);
        }
        Ok(events)
    }

    async fn delete(&self, session: &SessionId) -> Result<(), StoreError> {
        match fs::remove_file(self.file(session)) {
            Ok(()) => Ok(()),
            // Deleting an absent session is a no-op.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_err(error)),
        }
    }

    async fn list(&self) -> Result<Vec<SessionId>, StoreError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            // A store that has never been written to lists as empty.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_err(error)),
        };
        let mut sessions: Vec<(SessionId, std::time::SystemTime)> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_err)?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let modified = entry
                        .metadata()
                        .and_then(|meta| meta.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    sessions.push((SessionId::from(stem), modified));
                }
            }
        }
        // Most-recently-modified first.
        sessions.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
        Ok(sessions.into_iter().map(|(id, _)| id).collect())
    }

    async fn exists(&self, session: &SessionId) -> bool {
        self.file(session).exists()
    }

    async fn flush(&self, _session: &SessionId) -> Result<(), StoreError> {
        // Writes are unbuffered (each append opens, writes, closes), so there is
        // nothing to flush.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentx_domain::{ContentBlock, SessionEvent};
    use chrono::Utc;

    fn user_event(text: &str) -> PersistedEvent {
        PersistedEvent::new(
            SessionEvent::UserMessage {
                content: vec![ContentBlock::text(text)],
            },
            Utc::now(),
        )
    }

    #[tokio::test]
    async fn append_then_load_roundtrips_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let repo = FsSessionRepository::new(dir.path());
        let session = SessionId::from("s1");

        repo.append(&session, user_event("one")).await.unwrap();
        repo.append(&session, user_event("two")).await.unwrap();

        let loaded = repo.load(&session).await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].event, user_event("one").event);
        assert_eq!(loaded[1].event, user_event("two").event);
    }

    #[tokio::test]
    async fn exists_and_list_reflect_written_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let repo = FsSessionRepository::new(dir.path());
        let session = SessionId::from("abc");

        assert!(!repo.exists(&session).await);
        assert!(repo.list().await.unwrap().is_empty());

        repo.append(&session, user_event("hi")).await.unwrap();

        assert!(repo.exists(&session).await);
        assert_eq!(repo.list().await.unwrap(), vec![session]);
    }

    #[tokio::test]
    async fn delete_removes_history_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = FsSessionRepository::new(dir.path());
        let session = SessionId::from("gone");

        repo.append(&session, user_event("x")).await.unwrap();
        repo.delete(&session).await.unwrap();
        // Deleting again must not error.
        repo.delete(&session).await.unwrap();

        match repo.load(&session).await {
            Err(StoreError::NotFound(_)) => {}
            other => panic!("expected NotFound after delete, got {other:?}"),
        }
    }
}
