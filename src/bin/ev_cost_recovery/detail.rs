//! The Peak power detail tab: the three intervals of interest the delivery cost was priced on.
//!
//! Nothing is computed here. The run on the other tab already built these three, and what this tab
//! does is render them — so a figure shown here and the charge it produced cannot disagree.

use crate::{
    state::{SurplusState, WorkingDir},
    widgets,
};
use eframe::egui;
use ev_cost_recovery::{api::pure::PricedInterval, time::time_zone};
use jiff::Zoned;
use std::fs;

pub fn ui(ui: &mut egui::Ui, state: &mut SurplusState, working: &mut WorkingDir) {
    let Some(outcome) = &state.outcome else {
        return;
    };

    widgets::heading(ui, "Peak power detail");
    widgets::note(
        ui,
        "Three delivery charges are priced on peak power demand rather than on energy. One is \
         based on kVA, another on kW, and a third on 7-7 kW. Below is the relevant interval for \
         each charge, what the EV sessions are estimated to have contributed to it, and the \
         sessions running at the time.",
    );
    ui.add_space(12.0);

    let intervals = &outcome.surplus.delivery.priced_intervals;
    let text = document(intervals);
    let default_name = state.default_detail_save_name();
    export_row(ui, working, &text, &default_name);
    ui.add_space(12.0);

    // Plain text and open to begin with, which is how `widgets::section_ui` draws the other tabs'
    // report sections. These three are the same thing — a report broken into collapsible parts —
    // so they are drawn the same way. Colouring them and opening only the first made one report
    // look like a different kind of screen from the rest.
    for (i, priced) in intervals.iter().enumerate() {
        egui::CollapsingHeader::new(heading_for(priced))
            .id_salt(i)
            .default_open(true)
            .show(ui, |ui| {
                widgets::monospace_block(ui, priced.estimates.to_markdown().trim_end());
            });
    }
}

/// One interval's heading: the bill line's unit, then when the interval was, in local time.
///
/// Local rather than UTC because that is the clock the meter export's own hours are read on and the
/// one a reader checking against the bill is using.
fn heading_for(priced: &PricedInterval) -> String {
    let tz = time_zone();
    let start = Zoned::new(priced.estimates.interval.start, tz.clone());
    let end = Zoned::new(priced.estimates.interval.end(), tz);
    format!(
        "{} — {} to {}",
        priced.unit,
        start.strftime("%Y-%m-%d %H:%M"),
        end.strftime("%H:%M"),
    )
}

/// The three reports as one document, for saving and copying.
///
/// Each is rendered by the same `to_markdown` the command line prints, with a line above it saying
/// which of the three it is. That line is the only thing added.
fn document(intervals: &[PricedInterval; 3]) -> String {
    let mut out = String::new();
    for priced in intervals {
        out.push_str(&format!("Interval priced for {}\n\n", priced.unit));
        out.push_str(&priced.estimates.to_markdown());
        out.push('\n');
    }
    out
}

fn export_row(ui: &mut egui::Ui, working: &mut WorkingDir, text: &str, default_name: &str) {
    ui.horizontal(|ui| {
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(text.to_owned());
        }
        if ui.button("Save…").clicked()
            && let Some(path) = widgets::dialog(working)
                .set_file_name(default_name)
                .add_filter("Report", &["md"])
                .save_file()
        {
            working.remember(&path);
            if let Err(e) = fs::write(&path, text) {
                widgets::error_block(ui, &format!("{}: {e}", path.display()));
            }
        }
        widgets::note(ui, "All three intervals, as one document.");
    });
}
