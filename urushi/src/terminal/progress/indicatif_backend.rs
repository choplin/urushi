//! Private adapter from Urushi progress state to indicatif live regions.

use std::time::Duration;

use indicatif::{ProgressBar as IndicatifBar, ProgressDrawTarget, ProgressStyle};

const REFRESH_HZ: u8 = 20;
const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Debug)]
pub(super) struct LiveRegion {
    bar: IndicatifBar,
}

impl LiveRegion {
    pub(super) fn spinner(rail: &str, ticks: &[String], message: String) -> Self {
        let bar =
            IndicatifBar::with_draw_target(None, ProgressDrawTarget::stderr_with_hz(REFRESH_HZ));
        let template = format!("{rail}  {{spinner}} {{msg}}");
        let tick_refs = ticks.iter().map(String::as_str).collect::<Vec<_>>();
        let style = ProgressStyle::with_template(&template)
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&tick_refs);
        bar.set_style(style);
        bar.set_message(message);
        bar.enable_steady_tick(SPINNER_INTERVAL);
        Self { bar }
    }

    pub(super) fn progress(total: u64, rail: &str, progress: &str, message: String) -> Self {
        let bar = IndicatifBar::with_draw_target(
            Some(total),
            ProgressDrawTarget::stderr_with_hz(REFRESH_HZ),
        );
        let template = format!("{rail}  {progress} {{pos}}/{{len}} {{msg}}");
        let style = ProgressStyle::with_template(&template)
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("━╸─");
        bar.set_style(style);
        bar.set_message(message);
        Self { bar }
    }

    pub(super) fn set_message(&self, message: String) {
        self.bar.set_message(message);
    }

    pub(super) fn set_position(&self, position: u64) {
        self.bar.set_position(position);
    }

    pub(super) fn clear(self) {
        self.bar.finish_and_clear();
    }
}
