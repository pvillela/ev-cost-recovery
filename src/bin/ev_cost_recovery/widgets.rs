//! Small pieces of chrome the tabs share.
//!
//! Kept in the binary rather than the library: lifting window chrome into the library would put
//! `egui` in its public surface, and this is the only app that draws any.

use crate::{
    state::{RatesForm, Section, WorkingDir},
    theme::{self, Bold as _},
};
use eframe::egui;
use egui_extras::DatePickerButton;
use std::{fs, path::Path};

/// A file dialog that opens where the user last was, rather than wherever the system would put it.
pub fn dialog(working: &WorkingDir) -> rfd::FileDialog {
    // `set_directory` and nothing besides. On Linux rfd hands it to the XDG desktop portal as
    // `current_folder`, which the portal honours from 1.17 on; an older one opens in its own
    // default folder instead (see the `rfd` entry in `Cargo.toml`). Before anything has been
    // picked -- the first dialog of every run -- no folder is passed and the desktop's own choice
    // stands, since the app keeps nothing between runs.
    let dialog = rfd::FileDialog::new();
    match working.get() {
        Some(dir) => dialog.set_directory(dir),
        None => dialog,
    }
}

/// A tab's title, in the app's colour. Headings are the one place colour does structural work:
/// they say where a screen begins without a rule across the window.
///
/// Sized explicitly rather than left at egui's 18pt `heading`, which was smaller than the 20pt
/// figures the tabs state under it — the title of a screen read as subordinate to a line within it.
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .heading()
            .size(24.0)
            .bold()
            .color(theme::accent(ui)),
    );
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

/// A monospaced list whose lines are sentences rather than columns, so they wrap.
///
/// The counterpart to [`monospace_block`], and the difference is the whole point of having two:
/// report text is laid out to a fixed width and must not be re-wrapped, while an anomaly is a
/// sentence of no particular length and is simply lost off the right-hand edge if it is not.
pub fn monospace_lines(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap());
}

/// One schedule of cost-recovery rates: an effective date and the three bands. Returns whether any
/// of them was edited this frame.
///
/// `salt` distinguishes one schedule's widgets from another's on the same screen, which the surplus
/// tab needs for the rates a period changed to.
pub fn schedule(ui: &mut egui::Ui, form: &mut RatesForm, salt: &str) -> bool {
    let mut edited = false;
    egui::Grid::new(format!("rates_{salt}"))
        .spacing([12.0, 8.0])
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Effective from");
            let mut date = form.effective_date;
            if ui
                .add(DatePickerButton::new(&mut date).id_salt(&format!("effective_{salt}")))
                .changed()
                || date != form.effective_date
            {
                form.effective_date = date;
                edited = true;
            }
            ui.end_row();

            ui.label("Rates");
            ui.horizontal(|ui| {
                for (label, field) in [
                    ("On-peak", &mut form.on_peak),
                    ("Mid-peak", &mut form.mid_peak),
                    ("Off-peak", &mut form.off_peak),
                ] {
                    ui.label(label);
                    if ui
                        .add(
                            egui::TextEdit::singleline(field)
                                .desired_width(72.0)
                                .hint_text("0.0000"),
                        )
                        .changed()
                    {
                        edited = true;
                    }
                }
                ui.weak("$/kWh");
            });
            ui.end_row();
        });
    edited
}

/// One amount in a headline table.
///
/// `answer` colours it by its own sign. That is for the single figure the screen exists to state —
/// a surplus, a variance — and not for the figures it is made of, whose signs are only bookkeeping.
pub fn amount_label(ui: &mut egui::Ui, amount: f64, answer: bool) {
    // Colour, and nothing else. The amounts are aligned by padding them to twelve columns, which
    // holds only while every row is drawn in the same face, and `theme::Bold` swaps the family for
    // the bold one — which is the proportional list. A bold answer was therefore not monospace, and
    // its digits did not stand under the digits above it. There is no bold monospace face to reach
    // for, so the answer is set apart by colour alone.
    let mut text = egui::RichText::new(format!("{amount:>12.2}"))
        .monospace()
        .size(18.0);
    if answer {
        text = text.color(if amount < 0.0 {
            ui.visuals().error_fg_color
        } else {
            theme::accent(ui)
        });
    }
    ui.label(text);
}

/// The sections of a report as a tab shows them.
///
/// Every report opens with its own summary, which the tab drawing it has already stated above in
/// its own hand. Repeating it would give the answer twice, so that section stands down and the
/// sections nested in it take its place at the top, which is where they read from. Everything after
/// it is shown whole.
pub fn sections_to_show(mut sections: Vec<Section>) -> Vec<Section> {
    if sections.is_empty() {
        return sections;
    }
    let summary = sections.remove(0);
    let mut shown = summary.subsections;
    shown.extend(sections);
    shown
}

/// One section and everything nested in it, each with a heading that collapses.
///
/// Open to begin with, at every depth: a reader who has just asked for the figures wants to see
/// what they were drawn from, and collapsing is for putting a part away once it has been read.
pub fn section_ui(ui: &mut egui::Ui, section: &Section) {
    egui::CollapsingHeader::new(&section.title)
        .default_open(true)
        .show(ui, |ui| {
            // A section that nests others can hold nothing of its own, and an empty block would
            // draw a gap above the first subsection for no reason.
            if !section.body.is_empty() {
                monospace_block(ui, &section.body);
            }
            for sub in &section.subsections {
                section_ui(ui, sub);
            }
        });
}

/// The Copy and Save… row every report sits under.
///
/// One definition rather than one per tab: three tabs offered the same two buttons, and the only
/// thing that differed was the sentence beside them.
///
/// `save_error` is where a failed write is left, for the caller to draw *on the tab the save was
/// made from*. It is not the run's error: by the time there is a report to save, the figures are
/// worked out, and reporting a failed write as the run's failure says no result was produced when
/// one was. The Cost recovery and Peak power detail tabs are two views of one state, so a shared
/// slot would also put one tab's failed save under the other's button.
pub fn export_row(
    ui: &mut egui::Ui,
    working: &mut WorkingDir,
    text: &str,
    default_name: &str,
    note_text: &str,
    save_error: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(text.to_owned());
        }
        if ui.button("Save…").clicked()
            && let Some(path) = dialog(working)
                .set_file_name(default_name)
                .add_filter("Report", &["md"])
                .save_file()
        {
            // Remembered whether or not the write succeeds: it is where the user just chose to be
            // either way.
            working.remember(&path);
            // The saved file is byte-for-byte what the command line prints, so a report kept from
            // the app and one piped from the terminal are the same document.
            if let Err(e) = fs::write(&path, text) {
                *save_error = Some(format!("{}: {e}", path.display()));
            }
        }
        note(ui, note_text);
    });
}
