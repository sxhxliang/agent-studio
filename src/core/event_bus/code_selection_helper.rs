use agentx_event_bus::{CodeSelectionEvent, EventHub};
use agentx_types::events::CodeSelectionData;
use gpui::{App, Context, Entity};

/// Helper function to subscribe a panel entity to code selection events.
/// This reduces boilerplate by encapsulating the channel + background task pattern.
pub fn subscribe_entity_to_code_selections<T, F>(
    entity: &Entity<T>,
    event_hub: EventHub,
    panel_name: &'static str,
    on_selection: F,
    cx: &mut App,
) where
    T: 'static,
    F: Fn(&mut T, CodeSelectionData, &mut Context<T>) + 'static,
{
    let weak_entity = entity.downgrade();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CodeSelectionEvent>();

    log::info!("[{}] Subscribing to code selection events", panel_name);
    event_hub.subscribe_code_selections(move |event| {
        log::debug!(
            "[{}] Received selection: {}:{}~{}",
            panel_name,
            event.selection.file_path,
            event.selection.start_line,
            event.selection.end_line
        );
        let _ = tx.send(event.clone());
    });

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
