use std::sync::{Arc, Mutex};

use gpui::{App, Context, Entity};

use crate::app::actions::AddCodeSelection;

/// Event published when code is selected in the editor
#[derive(Clone, Debug)]
pub struct CodeSelectionEvent {
    pub selection: AddCodeSelection,
}

/// Event bus for broadcasting code selection events
pub struct CodeSelectionBus {
    subscribers: Vec<Box<dyn Fn(&CodeSelectionEvent) + Send + Sync>>,
}

impl CodeSelectionBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribe to code selection events
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&CodeSelectionEvent) + Send + Sync + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    /// Publish a code selection event to all subscribers
    pub fn publish(&self, event: CodeSelectionEvent) {
        log::info!(
            "[CodeSelectionBus] Publishing event - file: {}, lines: {}~{}",
            event.selection.file_path,
            event.selection.start_line,
            event.selection.end_line
        );

        for (idx, subscriber) in self.subscribers.iter().enumerate() {
            log::debug!("[CodeSelectionBus] Notifying subscriber {}", idx);
            subscriber(&event);
        }

        log::info!(
            "[CodeSelectionBus] Event published to {} subscribers",
            self.subscribers.len()
        );
    }
}

/// Thread-safe container for CodeSelectionBus
pub type CodeSelectionBusContainer = Arc<Mutex<CodeSelectionBus>>;

/// Helper function to subscribe a panel entity to code selection events
/// This reduces boilerplate by encapsulating the channel + background task pattern
///
/// # Arguments
/// * `entity` - The panel entity that will receive code selections
/// * `bus_container` - The global CodeSelectionBus container
/// * `panel_name` - Name for logging (e.g., "WelcomePanel", "ConversationPanel")
/// * `on_selection` - Callback to handle the code selection (receives mutable reference to panel)
/// * `cx` - GPUI App context
///
/// # Example
/// ```
/// subscribe_entity_to_code_selections(
///     &entity,
///     bus_container,
///     "MyPanel",
///     |panel, selection, cx| {
///         panel.code_selections.push(selection);
///         cx.notify();
///     },
///     cx
/// );
/// ```
pub fn subscribe_entity_to_code_selections<T, F>(
    entity: &Entity<T>,
    bus_container: CodeSelectionBusContainer,
    panel_name: &'static str,
    on_selection: F,
    cx: &mut App,
) where
    T: 'static,
    F: Fn(&mut T, AddCodeSelection, &mut Context<T>) + 'static,
{
    let weak_entity = entity.downgrade();

    // Create unbounded channel for cross-thread communication
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CodeSelectionEvent>();

    if let Ok(mut bus) = bus_container.lock() {
        log::info!("[{}] Subscribing to CodeSelectionBus", panel_name);

        bus.subscribe(move |event| {
            log::debug!(
                "[{}] Received selection: {}:{}~{}",
                panel_name,
                event.selection.file_path,
                event.selection.start_line,
                event.selection.end_line
            );
            let _ = tx.send(event.clone());
        });
    } else {
        log::error!("[{}] Failed to lock CodeSelectionBus", panel_name);
        return;
    }

    // Spawn background task
    cx.spawn(async move |cx| {
        while let Some(event) = rx.recv().await {
            if let Some(entity) = weak_entity.upgrade() {
                let _ = cx.update(|cx| {
                    entity.update(cx, |panel, cx| {
                        on_selection(panel, event.selection.clone(), cx);
                    });
                });
            } else {
                break;
            }
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_subscribe() {
        let mut bus = CodeSelectionBus::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        bus.subscribe(move |event| {
            received_clone
                .lock()
                .unwrap()
                .push(event.selection.file_path.clone());
        });

        bus.publish(CodeSelectionEvent {
            selection: AddCodeSelection {
                file_path: "test.rs".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 10,
                end_column: 1,
                content: "test content".to_string(),
            },
        });

        assert_eq!(received.lock().unwrap().len(), 1);
    }
}
