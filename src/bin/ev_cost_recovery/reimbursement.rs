//! The Reimbursement tab: one month's session report and what Evolute paid, the variance out.
//!
//! Standalone. It reads no bill and no meter export, and shares nothing with the other two tabs but
//! the folder the file dialogs open in — the question is whether Evolute paid what our rates earned
//! over a calendar month, not whether those rates cover Toronto Hydro's bill over a billing period.

use crate::{
    state::{ReimbursementState, WorkingDir, report_sections},
    theme::{self, Bold as _},
    widgets,
};
use eframe::egui;
use ev_cost_recovery::api::ReimbursementReconciliation;
use std::fs;

pub fn ui(ui: &mut egui::Ui, state: &mut ReimbursementState, working: &mut WorkingDir) {
    widgets::heading(ui, "Evolute reimbursement reconciliation");
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

/// Wide enough for the longest label on this tab. Both grids are given it so their label columns
/// line up, which they would not do on their own: a note sits between them, and each would
/// otherwise size its column to its own labels.
const LABEL_WIDTH: f32 = 150.0;

/// The report and the amount, in the order they are asked for: the report says which month this is,
/// and the amount is what that month was paid.
///
/// Two grids rather than one, so each note sits directly beneath the field it is about. They ask
/// for different things from different documents, and a single note at the foot left a reader to
/// work out which sentence was about which field.
fn inputs(ui: &mut egui::Ui, state: &mut ReimbursementState, working: &mut WorkingDir) {
    egui::Grid::new("reimbursement_files")
        .spacing([12.0, 8.0])
        .min_col_width(LABEL_WIDTH)
        .num_columns(3)
        .show(ui, |ui| {
            ui.label("Evolute Session Report");
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

            ui.label("Evolute Charges Report");
            if ui.button("Choose…").clicked()
                && let Some(path) = widgets::dialog(working)
                    .add_filter("Charges Report", &["csv", "CSV"])
                    .pick_file()
            {
                working.remember(&path);
                state.select_charges(path);
            }
            widgets::picked_file(ui, state.charges.as_deref(), "None chosen");
            ui.end_row();
        });

    // Kept when the rest of this note went, because it is the one thing here that is not visible
    // from the screen: both files are chosen by hand, and a refusal nobody was warned of reads as
    // a fault rather than as the check it is.
    widgets::note(
        ui,
        "If the two files turn out to cover different months, the reconciliation is refused rather \
         than run.",
    );

    ui.add_space(10.0);

    // Collected across the grid and acted on after it, because `state` is borrowed field by field
    // inside and `edited` takes the whole of it.
    let mut edited = false;
    egui::Grid::new("reimbursement_figures")
        .spacing([12.0, 8.0])
        .min_col_width(LABEL_WIDTH)
        .num_columns(3)
        .show(ui, |ui| {
            ui.label("Reimbursement");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut state.reimbursed)
                        .desired_width(96.0)
                        .hint_text("0.00"),
                )
                .changed()
            {
                edited = true;
            }
            ui.weak("$ actually received for the month");
            ui.end_row();
        });

    if edited {
        state.edited();
    }

    widgets::note(
        ui,
        "The one figure entered manually, because it is not in either document: it is what was \
         seen to arrive, from a bank statement or a remittance advice.",
    );
}

fn rates(ui: &mut egui::Ui, state: &mut ReimbursementState) {
    ui.label(egui::RichText::new("Cost-recovery rates").bold());
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

/// The two money subtractions, in the order the report's own summary states them, so the headline
/// and the tables cannot tell different stories.
///
/// Money only. The energy variance is a supporting figure and sits below with the table it is
/// drawn from — a column of figures in two units invites reading one as the other, and it answers
/// neither of the questions this screen exists to settle.
fn headline(ui: &mut egui::Ui, r: &ReimbursementReconciliation) {
    let rows = |ui: &mut egui::Ui, salt: &str, unit: &str, rows: [(&str, f64, bool); 3]| {
        egui::Grid::new(format!("reimbursement_headline_{salt}"))
            .spacing([28.0, 6.0])
            .num_columns(3)
            .show(ui, |ui| {
                for (label, amount, answer) in rows {
                    let text = egui::RichText::new(label);
                    ui.label(if answer { text.bold() } else { text });
                    widgets::amount_label(ui, amount, answer);
                    ui.weak(unit);
                    ui.end_row();
                }
            });
    };

    // The second figure is negative in each column so it adds down to the variance. A subtraction
    // cannot be checked against two positive numbers.
    //
    // The remittance first, because it is the narrower question and the one that has to hold
    // before the other means anything. The Charges Report total appears in it and nowhere after:
    // repeating it beside a subtraction that does not use it invites reading one as the other.
    rows(
        ui,
        "remittance",
        "$",
        [
            ("Reimbursement received", r.reimbursed, false),
            ("Charges Report total", -r.charges_report_amount, false),
            ("Remittance variance", r.remittance_variance, true),
        ],
    );
    ui.add_space(10.0);
    rows(
        ui,
        "money",
        "$",
        [
            ("Reimbursement received", r.reimbursed, false),
            ("Cost recovery earned", -r.cost_recovery_amount, false),
            ("Dollar variance", r.dollar_variance, true),
        ],
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
