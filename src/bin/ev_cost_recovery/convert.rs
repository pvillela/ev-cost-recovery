//! The Convert tab: a source file in, a workbook beside it out.
//!
//! Neither conversion feeds anything else in the app — the other tabs read the source files
//! themselves. These workbooks are for reading by eye and for checking a figure against an invoice
//! by hand, which is why an existing one is worth asking about before it is replaced.

use crate::{
    state::{
        Conversion, ConversionSlot, ConvertState, GbConversion, GbWorkbook, SessionConversion,
        SessionWorkbook, Which, WorkingDir,
    },
    theme::{self, Bold as _},
    widgets,
};
use eframe::egui;
use ev_cost_recovery::api::OnExistingWorkbook;
use std::path::Path;

pub fn ui(ui: &mut egui::Ui, state: &mut ConvertState, working: &mut WorkingDir) {
    widgets::heading(ui, "Convert a file to a workbook");
    widgets::note(
        ui,
        "Turns a source file into an Excel workbook beside it, with the columns this software \
         derives and anything that needed a judgement call marked in the sheet. Nothing on the \
         other tabs depends on these: they read the source files themselves. The workbooks can be \
         generated to facilitate inspection and exploration of the source data.",
    );
    ui.add_space(14.0);

    chooser(ui, &mut state.which);
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(14.0);

    // One at a time. Drawn one above the other, a long result for the first pushed the second's
    // button off the screen, so reaching it meant scrolling past a report that had nothing to do
    // with it.
    match state.which {
        Which::Sessions => {
            picker::<SessionConversion>(
                ui,
                &mut state.sessions,
                working,
                "Evolute session report",
                "Session report",
                &["csv", "CSV"],
                "One row per charging session: the report's own columns in the order it states \
                 them, then the derived ones, with the adjusted duration and the average kW as \
                 live formulas. Every session is written, anomalous ones included.",
            );
            if let Some(outcome) = &state.sessions.outcome {
                session_outcome(ui, outcome);
            }
        }
        Which::GreenButton => {
            picker::<GbConversion>(
                ui,
                &mut state.green_button,
                working,
                "Toronto Hydro Green Button export",
                "Green Button export",
                &["xml", "XML"],
                "The workbook contains two sheets. Peak_values carries one row per billing period \
                 — the energy used, the highest kW and kVA over the period and within the 7-7 \
                 demand window, when each occurred, and in which Time-of-Use period. \
                 Interval_values carries every hour of the export. A multi-year export takes a \
                 moment to parse.",
            );
            if let Some(outcome) = &state.green_button.outcome {
                gb_outcome(ui, outcome);
            }
        }
    }
}

/// Which conversion is on screen.
///
/// A row of selectable buttons rather than a second row of tabs in the bar above. These two are one
/// job seen twice, not two places in the app, and promoting them would put four entries in a bar
/// that holds four already. Each keeps whatever it has done: switching away and back finds the
/// file still chosen and the last result still there.
fn chooser(ui: &mut egui::Ui, which: &mut Which) {
    ui.horizontal(|ui| {
        ui.label("Convert");
        for (value, label) in [
            (Which::Sessions, "Session report"),
            (Which::GreenButton, "Green Button export"),
        ] {
            let selected = *which == value;
            let text = egui::RichText::new(label);
            let text = if selected {
                text.bold().color(theme::accent(ui))
            } else {
                text
            };
            if ui.add(egui::Button::selectable(selected, text)).clicked() {
                *which = value;
            }
        }
    });
}

/// One conversion's file picker, its Convert button, its error and its replace prompt.
///
/// Generic over the conversion for the reason [`Conversion`] exists: the only thing that differs
/// between the two halves of this tab is the wording and the file filter.
fn picker<C: Conversion>(
    ui: &mut egui::Ui,
    slot: &mut ConversionSlot<C>,
    working: &mut WorkingDir,
    title: &str,
    filter_name: &str,
    // Both cases of every extension: a Linux dialog matches the extension as the glob `*.csv`,
    // which a `.CSV` file does not answer to.
    extensions: &[&str],
    note: &str,
) {
    ui.label(egui::RichText::new(title).bold());
    widgets::note(ui, note);
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui.button("Choose…").clicked()
            && let Some(path) = widgets::dialog(working)
                .add_filter(filter_name, extensions)
                .pick_file()
        {
            working.remember(&path);
            slot.select(path);
        }
        widgets::picked_file(ui, slot.input.as_deref(), "None chosen");
    });

    ui.add_space(10.0);
    if ui
        .add_enabled(slot.input.is_some(), egui::Button::new("Convert"))
        .clicked()
    {
        slot.start();
    }

    if let Some(message) = &slot.error {
        ui.add_space(8.0);
        widgets::error_block(ui, message);
    }

    replace_prompt(ui, slot);
}

/// Asked before a workbook is replaced.
///
/// The api refuses an existing workbook unless it is told to replace it, and this is the only place
/// that tells it to. A workbook may have been reconciled against an invoice by hand, and that work
/// is not in any source file.
fn replace_prompt<C: Conversion>(ui: &mut egui::Ui, slot: &mut ConversionSlot<C>) {
    let Some(target) = slot.confirm_replace.clone() else {
        return;
    };
    // Salted by the file, so the two conversions cannot both claim the same modal id.
    egui::Modal::new(egui::Id::new(("confirm_replace", target.clone()))).show(ui.ctx(), |ui| {
        ui.set_max_width(460.0);
        ui.heading("Replace existing workbook?");
        ui.add_space(8.0);
        ui.add(egui::Label::new(target.display().to_string()).wrap());
        ui.add_space(8.0);
        widgets::note(
            ui,
            "This file already exists. Converting again overwrites it, and anything written into \
             the old workbook by hand goes with it.",
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Replace").clicked() {
                slot.convert(OnExistingWorkbook::Replace);
            }
            if ui.button("Cancel").clicked() {
                slot.confirm_replace = None;
            }
        });
    });
}

/// Where the workbook went. The same line for both conversions, so they read alike.
fn written(ui: &mut egui::Ui, path: &Path) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new("Workbook written").bold());
    ui.add(egui::Label::new(path.display().to_string()).wrap());
}

fn session_outcome(ui: &mut egui::Ui, outcome: &SessionWorkbook) {
    written(ui, &outcome.workbook);

    // Beneath the workbook's path, not above it. The workbook was written; this says only that its
    // log was not, and reads as a caveat on the line before it rather than as a failed conversion.
    if let Some(message) = &outcome.log_failure {
        ui.add_space(8.0);
        widgets::error_block(ui, message);
    }

    ui.add_space(10.0);
    if outcome.anomalies.is_empty() {
        widgets::note(ui, "No row needed a judgement call.");
        return;
    }
    ui.label(egui::RichText::new(format!(
        "{} row(s) needed a judgement call",
        outcome.anomalies.len()
    )));
    widgets::note(
        ui,
        "These are recorded in the workbook's Anomalies column and do not stop the conversion. Row \
         numbers are rows of the CSV, so a record duplicated to resolve a DST fold appears twice \
         against the one row it came from; the -EDT/-EST suffix on the session id tells the two \
         apart.",
    );
    ui.add_space(6.0);
    widgets::monospace_lines(ui, &outcome.anomalies.join("\n"));
}

fn gb_outcome(ui: &mut egui::Ui, gb: &GbWorkbook) {
    let outcome = &gb.report;
    written(ui, &outcome.path);

    // Beneath the workbook's path, for the reason `session_outcome` gives.
    if let Some(message) = &gb.log_failure {
        ui.add_space(8.0);
        widgets::error_block(ui, message);
    }

    ui.add_space(10.0);
    ui.label(format!(
        "{} billing periods, {} intervals",
        outcome.period_rows, outcome.interval_rows
    ));

    if outcome.incomplete_periods > 0 {
        ui.add_space(6.0);
        ui.label(format!(
            "{} period(s) do not hold a full billing period's intervals",
            outcome.incomplete_periods
        ));
        widgets::note(
            ui,
            "Highlighted in the sheet. The export's own coverage decides this: the first and last \
             periods it reaches are ordinarily partial.",
        );
    }

    if !outcome.anomaly_counts.is_empty() {
        ui.add_space(6.0);
        let counts: Vec<String> = outcome
            .anomaly_counts
            .iter()
            .map(|(kind, count)| format!("{kind} x{count}"))
            .collect();
        ui.label(egui::RichText::new("Anomalies").bold());
        widgets::note(
            ui,
            "Highlighted in the sheet, against the readings they concern. They do not stop the \
             conversion.",
        );
        ui.add_space(6.0);
        widgets::monospace_lines(ui, &counts.join("\n"));
    }
}
