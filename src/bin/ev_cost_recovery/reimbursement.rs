//! The Reimbursement tab: one month's session report and what Evolute paid, the variance out.
//!
//! Standalone. It reads no bill and no meter export, and shares nothing with the other two tabs but
//! the folder the file dialogs open in — the question is whether Evolute paid what our rates earned
//! over a calendar month, not whether those rates cover Toronto Hydro's bill over a billing period.

use crate::{
    state::{ReimbursementState, WorkingDir, report_sections},
    theme, widgets,
};
use eframe::egui;
use ev_cost_recovery::io::ReimbursementReconciliation;
use std::fs;

pub fn ui(ui: &mut egui::Ui, state: &mut ReimbursementState, working: &mut WorkingDir) {
    widgets::heading(ui, "Evolute reimbursement");
    widgets::note(
        ui,
        "What the cost-recovery rates earn over one calendar month, against what Evolute actually \
         paid for it. A calendar month, not a billing period: that is what Evolute reports and \
         settles on, so this figure and the surplus on the other tab cover different days and are \
         not two views of one number.",
    );
    ui.add_space(12.0);

    inputs(ui, state, working);
    ui.add_space(14.0);
    rates(ui, state);

    ui.add_space(14.0);
    if ui
        .add_enabled(state.can_run(), egui::Button::new("Reconcile the month"))
        .clicked()
    {
        state.run();
    }

    if let Some(message) = &state.error {
        ui.add_space(8.0);
        widgets::error_block(ui, message);
    }

    if state.outcome.is_some() {
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        results(ui, state, working);
    }
}

/// The report and the amount, in the order they are asked for: the report says which month this is,
/// and the amount is what that month was paid.
fn inputs(ui: &mut egui::Ui, state: &mut ReimbursementState, working: &mut WorkingDir) {
    // Collected across the grid and acted on after it, because `state` is borrowed field by field
    // inside and `edited` takes the whole of it.
    let mut edited = false;
    egui::Grid::new("reimbursement_inputs")
        .spacing([12.0, 8.0])
        .num_columns(3)
        .show(ui, |ui| {
            ui.label("Session report");
            if ui.button("Choose…").clicked()
                && let Some(path) = widgets::dialog(working)
                    .add_filter("Session report", &["csv", "CSV"])
                    .pick_file()
            {
                working.remember(&path);
                state.select(path);
            }
            widgets::picked_file(ui, state.sessions.as_deref(), "None chosen");
            ui.end_row();

            if let Some(note) = &state.input_note {
                ui.label("");
                ui.label("");
                widgets::error_block(ui, note);
                ui.end_row();
            }

            // Both come off Evolute's Charges Report, which the app does not read, so they are
            // typed. They are the other side of the comparison: the session report supplies ours.
            for (label, field, hint, unit) in [
                (
                    "Charges Report kWh",
                    &mut state.charges_report_kwh,
                    "0.000",
                    "kWh Evolute billed the month on",
                ),
                (
                    "Reimbursement",
                    &mut state.reimbursement,
                    "0.00",
                    "$ received for the month",
                ),
            ] {
                ui.label(label);
                if ui
                    .add(
                        egui::TextEdit::singleline(field)
                            .desired_width(96.0)
                            .hint_text(hint),
                    )
                    .changed()
                {
                    edited = true;
                }
                ui.weak(unit);
                ui.end_row();
            }
        });

    if edited {
        state.edited();
    }

    widgets::note(
        ui,
        "One report, covering one whole calendar month. Its file name is the only thing that says \
         which month that is. The two figures beneath it are read off Evolute's Charges Report, \
         which is a different document and is not opened here.",
    );
}

fn rates(ui: &mut egui::Ui, state: &mut ReimbursementState) {
    ui.label(egui::RichText::new("Cost-recovery rates").strong());
    widgets::note(
        ui,
        "The rates in effect over the month, in dollars per kilowatt-hour. One schedule only: our \
         rates change on the first of a month, so a month has one set of them.",
    );
    ui.add_space(6.0);

    if widgets::schedule(ui, &mut state.rates, "reimbursement") {
        state.edited();
    }
}

// --------------------------------------------------------------------------------------------
// Results

fn results(ui: &mut egui::Ui, state: &mut ReimbursementState, working: &mut WorkingDir) {
    let Some(outcome) = &state.outcome else {
        return;
    };
    let text = outcome.text.clone();
    let default_name = state.default_save_name();

    ui.label(
        egui::RichText::new(
            outcome
                .reconciliation
                .month_start
                .strftime("%B %Y")
                .to_string(),
        )
        .heading()
        .size(20.0)
        .color(theme::accent(ui)),
    );
    ui.add_space(10.0);
    headline(ui, &outcome.reconciliation);

    ui.add_space(14.0);
    export_row(ui, state, working, &text, &default_name);
    ui.add_space(10.0);

    for section in widgets::sections_to_show(report_sections(&text)) {
        widgets::section_ui(ui, &section);
    }
}

/// Both subtractions, each in its own unit, in the order the report's own summary states them, so
/// the headline and the tables cannot tell different stories.
///
/// Money and energy in two grids rather than one. They answer different questions — was the month
/// paid for, and do the two documents agree about how much was drawn — and a single column of
/// figures in two units invites reading one as the other.
fn headline(ui: &mut egui::Ui, r: &ReimbursementReconciliation) {
    let rows = |ui: &mut egui::Ui, salt: &str, unit: &str, rows: [(&str, f64, bool); 3]| {
        egui::Grid::new(format!("reimbursement_headline_{salt}"))
            .spacing([28.0, 6.0])
            .num_columns(3)
            .show(ui, |ui| {
                for (label, amount, answer) in rows {
                    let text = egui::RichText::new(label);
                    ui.label(if answer { text.strong() } else { text });
                    widgets::amount_label(ui, amount, answer);
                    ui.weak(unit);
                    ui.end_row();
                }
            });
    };

    // The costs are negative in each column so it adds down to the variance. A subtraction cannot
    // be checked against two positive numbers.
    rows(
        ui,
        "money",
        "$",
        [
            ("Reimbursement received", r.reimbursement, false),
            ("Cost recovery earned", -r.cost_recovery_amount, false),
            ("Dollar variance", r.dollar_variance, true),
        ],
    );
    ui.add_space(10.0);
    rows(
        ui,
        "energy",
        "kWh",
        [
            ("On Evolute's Charges Report", r.charges_report_kwh, false),
            (
                "Priced from the session report",
                -r.tou_kwh.total_kwh(),
                false,
            ),
            ("Energy variance", r.kwh_variance, true),
        ],
    );

    ui.add_space(8.0);
    widgets::note(
        ui,
        "A negative dollar variance is Evolute having paid less than the rates come to for the \
         month. The energy variance sets our figure against Evolute's, which come from different \
         documents and are arrived at differently. Nothing on Toronto Hydro's bill is counted \
         anywhere here.",
    );
}

fn export_row(
    ui: &mut egui::Ui,
    state: &mut ReimbursementState,
    working: &mut WorkingDir,
    text: &str,
    default_name: &str,
) {
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
            // Remembered whether or not the write succeeds: it is where the user just chose to be
            // either way.
            working.remember(&path);
            if let Err(e) = fs::write(&path, text) {
                state.error = Some(format!("{}: {e}", path.display()));
            }
        }
        widgets::note(ui, "The full reconciliation, as one document.");
    });
}
