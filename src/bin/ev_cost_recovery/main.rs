//! The desktop app for the EV cost-recovery surplus: what the rates recover, less the chargers'
//! share of the bill.
//!
//! The figures are the same as those from `cost_recovery_surplus_cli`; computed by the same library
//! code, and a saved report is byte-for-byte what that command prints. What differs is only the
//! asking: four files chosen from pickers and the rates typed into a form, rather than six
//! positional arguments in which a schedule is written `DATE:ON,MID,OFF`.
//!
//! The second tab is what the command line cannot show: the three intervals of interest the
//! delivery cost was priced on, each with the segment and the sessions behind its figure.

// A GUI has no use for a console, and on Windows one would open behind the window. Kept in debug
// builds, where a panic message on stderr is worth more than the tidiness.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about;
mod app;
mod convert;
mod detail;
mod reimbursement;
mod state;
mod surplus;
mod theme;
mod widgets;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(app::APP_NAME)
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([560.0, 420.0])
            .with_icon(app::icon()),
        ..Default::default()
    };

    eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(|cc| {
            // egui follows the system light/dark setting on its own; all that is wanted here is
            // slightly larger text than the default, on top of whatever the display's scaling is.
            cc.egui_ctx.set_zoom_factor(1.15);
            theme::apply(&cc.egui_ctx);
            Ok(Box::<app::App>::default())
        }),
    )
}
