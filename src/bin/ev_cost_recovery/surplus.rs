//! The Cost recovery tab: four files and a rate schedule in, the surplus report out.

use crate::{
    state::{Input, SurplusState, WorkingDir, report_sections},
    theme, widgets,
};
use eframe::egui;
use ev_cost_recovery::io::CostRecoverySurplus;
use std::fs;

pub fn ui(ui: &mut egui::Ui, state: &mut SurplusState, working: &mut WorkingDir) {
    widgets::heading(ui, "EV cost recovery surplus");
    widgets::note(
        ui,
        "The amount recovered over the billing period by the EV cost-recovery rates, minus the \
         energy and delivery costs attributable to EV charging sessions.",
    );
    ui.add_space(12.0);

    inputs(ui, state, working);
    ui.add_space(14.0);
    rates(ui, state);

    ui.add_space(14.0);
    if ui
        .add_enabled(state.can_run(), egui::Button::new("Work out the surplus"))
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

/// The four pickers, in the order the run reads them.
///
/// The bill first because it is the one that says which billing period this is; no closing date is
/// asked for anywhere, and the other three are read against what the bill says.
fn inputs(ui: &mut egui::Ui, state: &mut SurplusState, working: &mut WorkingDir) {
    // A grid rather than four rows, so the buttons line up under each other: the labels differ in
    // length, and ragged buttons read as four unrelated controls rather than one list.
    egui::Grid::new("inputs")
        .spacing([12.0, 8.0])
        .num_columns(3)
        .show(ui, |ui| {
            for which in Input::ALL {
                ui.label(which.label());
                if ui.button("Choose…").clicked() {
                    let (description, extensions) = which.filter();
                    if let Some(path) = widgets::dialog(working)
                        .add_filter(description, extensions)
                        .pick_file()
                    {
                        working.remember(&path);
                        state.select(which, path);
                    }
                }
                widgets::picked_file(ui, state.picked(which), "None chosen");
                ui.end_row();

                // Said against the picker it belongs to, while the dialog is still fresh in mind.
                if let Some(note) = state.note_for(which) {
                    ui.label("");
                    ui.label("");
                    widgets::error_block(ui, note);
                    ui.end_row();
                }
            }
        });

    widgets::note(
        ui,
        "A billing period runs from the 24th to the 23rd, so it spans two monthly session reports. \
         Either order will do — the names say what each holds.",
    );
}

fn rates(ui: &mut egui::Ui, state: &mut SurplusState) {
    ui.label(egui::RichText::new("Cost-recovery rates").strong());
    widgets::note(ui, "In dollars per kilowatt-hour.");
    ui.add_space(6.0);

    if widgets::schedule(ui, &mut state.rates_at_start, "start") {
        state.rates_edited();
    }

    ui.add_space(6.0);
    let mut changed = state.rates_changed;
    if ui
        .checkbox(&mut changed, "The rates changed during the period")
        .changed()
    {
        state.set_rates_changed(changed);
    }

    if state.rates_changed {
        ui.add_space(6.0);
        // The energy is split at local midnight on the second schedule's effective date, so that
        // date has to fall inside the period. The library says so if it does not.
        if widgets::schedule(ui, &mut state.rates_at_end, "end") {
            state.rates_edited();
        }
    }
}

// --------------------------------------------------------------------------------------------
// Results

fn results(ui: &mut egui::Ui, state: &mut SurplusState, working: &mut WorkingDir) {
    let Some(outcome) = &state.outcome else {
        return;
    };
    let text = outcome.text.clone();
    let default_name = state.default_save_name();

    ui.label(
        egui::RichText::new(format!(
            "Billing period ending {}",
            outcome.surplus.recovery.billing_period_ending
        ))
        .heading()
        .size(20.0)
        .color(theme::accent(ui)),
    );
    ui.add_space(10.0);
    headline(ui, &outcome.surplus);

    ui.add_space(14.0);
    export_row(ui, state, working, &text, &default_name);
    ui.add_space(10.0);

    for section in widgets::sections_to_show(report_sections(&text)) {
        widgets::section_ui(ui, &section);
    }
}

/// The four amounts of the subtraction, in the order the report's summary table states them, so the
/// headline and the table cannot tell different stories.
fn headline(ui: &mut egui::Ui, surplus: &CostRecoverySurplus) {
    egui::Grid::new("headline")
        .spacing([28.0, 6.0])
        .num_columns(2)
        .show(ui, |ui| {
            for (label, amount, strong) in [
                ("Cost recovery", surplus.recovery.cost_recovery, false),
                ("EV energy cost", -surplus.energy.energy_cost, false),
                ("EV delivery cost", -surplus.delivery.delivery_cost, false),
                ("Surplus", surplus.surplus, true),
            ] {
                let text = egui::RichText::new(label);
                ui.label(if strong { text.strong() } else { text });
                widgets::amount_label(ui, amount, strong);
                ui.end_row();
            }
        });
}

fn export_row(
    ui: &mut egui::Ui,
    state: &mut SurplusState,
    working: &mut WorkingDir,
    text: &str,
    default_name: &str,
) {
    ui.horizontal(|ui| {
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(text.to_owned());
        }
        if ui.button("Save…").clicked() {
            // The saved file is byte-for-byte what the command line prints, so a report kept from
            // the app and one piped from the terminal are the same document.
            if let Some(path) = widgets::dialog(working)
                .set_file_name(default_name)
                .add_filter("Report", &["md"])
                .save_file()
            {
                // Remembered whether or not the write succeeds: it is where the user just chose to
                // be either way.
                working.remember(&path);
                if let Err(e) = fs::write(&path, text) {
                    state.error = Some(format!("{}: {e}", path.display()));
                }
            }
        }
        widgets::note(
            ui,
            "The full report, exactly as the command line prints it.",
        );
    });
}
