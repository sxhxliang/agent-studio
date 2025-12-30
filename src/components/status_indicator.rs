use gpui::{IntoElement, RenderOnce, Styled, Window, div, px};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::services::SessionStatus;

/// A status indicator dot that shows different colors for different statuses
#[derive(IntoElement)]
pub struct StatusIndicator {
    status: SessionStatus,
    size: f32,
    opacity: Option<f32>,
}

impl StatusIndicator {
    pub fn new(status: SessionStatus) -> Self {
        Self {
            status,
            size: 8.0,
            opacity: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity);
        self
    }

    fn status_color(&self) -> gpui::Hsla {
        match self.status {
            SessionStatus::Active => gpui::rgb(0x22c55e).into(),
            SessionStatus::Idle => gpui::rgb(0x22c55e).into(),
            SessionStatus::Pending => gpui::rgb(0x6b7280).into(),
            SessionStatus::InProgress => gpui::rgb(0x3b82f6).into(),
            SessionStatus::Completed => gpui::rgb(0x22c55e).into(),
            SessionStatus::Failed => gpui::rgb(0xef4444).into(),
            SessionStatus::Closed => gpui::rgb(0x6b7280).into(),
        }
    }

    fn should_pulse(&self) -> bool {
        matches!(
            self.status,
            SessionStatus::InProgress | SessionStatus::Pending
        )
    }

    fn calculate_pulse_opacity() -> f32 {
        // Calculate opacity based on current time for smooth pulsing
        const PULSE_DURATION_MS: u64 = 1500; // 1.5 seconds for one full pulse cycle

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let elapsed_ms = now.as_millis() as u64;

        let phase = (elapsed_ms % PULSE_DURATION_MS) as f32 / PULSE_DURATION_MS as f32;
        let angle = phase * 2.0 * std::f32::consts::PI;

        // Oscillate between 0.3 and 1.0 opacity
        0.3 + (0.7 * ((angle.sin() + 1.0) / 2.0))
    }
}

impl RenderOnce for StatusIndicator {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let color = self.status_color();
        let size_px = px(self.size);

        // Use provided opacity or calculate pulse opacity if needed
        let opacity = self.opacity.unwrap_or_else(|| {
            if self.should_pulse() {
                Self::calculate_pulse_opacity()
            } else {
                1.0
            }
        });

        div()
            .flex_shrink_0()
            .w(size_px)
            .h(size_px)
            .rounded(size_px / 2.0) // Make it circular
            .bg(color)
            .opacity(opacity)
    }
}
