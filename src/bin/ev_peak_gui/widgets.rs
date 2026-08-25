//! Small pieces of chrome the two tabs share.

use crate::{state::WorkingDir, theme};
use eframe::egui;
use std::path::Path;

/// A file dialog that opens where the user last was, rather than wherever the system would put it.
pub fn dialog(working: &WorkingDir) -> rfd::FileDialog {
    // `set_directory` and nothing besides. It holds on Linux only because `Cargo.toml` picks
    // rfd's GTK backend there; reached through the XDG desktop portal the folder is discarded on
    // every version before 1.17. Do not pass the folder a second time as a file name to
    // compensate: GTK reads a file name as a file to select, and a folder is not one.
    let dialog = rfd::FileDialog::new();
    match working.get() {
        Some(dir) => dialog.set_directory(dir),
        None => dialog,
    }
}

/// A tab's title, in the app's colour. Headings are the one place colour does structural work:
/// they say where a screen begins without a rule across the window.
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).heading().color(theme::accent(ui)));
}

/// The colour a failure is shown in, in whichever theme is on.
fn error_color(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().error_fg_color
}

/// A failure, shown where the action that failed was taken.
///
/// The text is the library's own message, unaltered, so that trouble reported from the app and
/// trouble reported from the command line can be compared word for word.
pub fn error_block(ui: &mut egui::Ui, message: &str) {
    let color = error_color(ui);
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, color))
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.colored_label(color, "⚠");
                ui.add(egui::Label::new(egui::RichText::new(message).color(color)).wrap());
            });
        });
}

/// The file a picker has in hand, shown beside its button.
///
/// The name, not the path: the folder is what the user just navigated, so repeating it says little
/// and a deep path pushes everything else off the row. The whole path is a hover away for the times
/// it matters — two workbooks a month apart can have the same name.
pub fn picked_file(ui: &mut egui::Ui, path: Option<&Path>, none_text: &str) {
    match path {
        Some(path) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            ui.label(name).on_hover_text(path.display().to_string());
        }
        None => {
            ui.weak(none_text);
        }
    }
}

/// A note that is not a failure: what was written, what was found, what is not there.
pub fn note(ui: &mut egui::Ui, message: &str) {
    ui.add(egui::Label::new(egui::RichText::new(message).weak()).wrap());
}

/// Report text, shown exactly as the command line prints it.
pub fn monospace_block(ui: &mut egui::Ui, text: &str) {
    // Labels are selectable, so the text can still be picked up by hand. `Extend` keeps the
    // report's own wrapping — it is written to a fixed width and re-wrapping would break its
    // tables.
    ui.add(
        egui::Label::new(egui::RichText::new(text).monospace())
            .wrap_mode(egui::TextWrapMode::Extend),
    );
}

/// A row of mutually exclusive choices, drawn as buttons rather than radio dots.
///
/// Returns the choice made this frame, if any. A choice that is not `enabled` is drawn greyed:
/// an interval of one hour from `HH:15` is not an error to be explained, it is simply not on
/// offer.
pub fn choice_row<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: T,
    options: &[(T, &str, bool)],
) -> Option<T> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        for &(value, label, enabled) in options {
            let selected = value == current;
            let response = ui.add_enabled(enabled, egui::Button::selectable(selected, label));
            if response.clicked() && !selected {
                chosen = Some(value);
            }
        }
    });
    chosen
}
