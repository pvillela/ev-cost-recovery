//! The window: the tab bar and which tab is drawn.

use crate::{
    about, convert, detail, reimbursement,
    state::{AppState, Tab},
    surplus,
    theme::{self, Bold as _},
};
use eframe::egui;

pub const APP_NAME: &str = "EV Cost Recovery";

/// The window and taskbar icon, and the mark shown in the corner of the tab bar.
///
/// Falling back to an empty icon is right if it ever fails to decode: an app without an icon is a
/// great deal better than an app that will not start.
pub fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon.png"))
        .unwrap_or_default()
}

/// The mark's side in points. Short enough to sit inside the tab bar without setting its height,
/// which the buttons beside it do.
const LOGO_SIDE: f32 = 24.0;

#[derive(Default)]
pub struct App {
    state: AppState,
    /// Uploaded on first draw rather than at startup, so nothing is paid for it until there is a
    /// window to put it in.
    logo: Option<egui::TextureHandle>,
    /// Window chrome rather than workflow state, so it sits here and not in `AppState`: opening the
    /// About window is not a step in the work and must not survive as one.
    about_open: bool,
}

impl App {
    /// The mark, as a texture the renderer can draw. Uploaded once and handed out by clone
    /// thereafter.
    fn logo_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        self.logo
            .get_or_insert_with(|| {
                let icon = icon();
                let size = [icon.width as usize, icon.height as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
                ctx.load_texture("logo", image, egui::TextureOptions::LINEAR)
            })
            .clone()
    }
}

impl eframe::App for App {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Taken before the panel is drawn, because the closure that draws it borrows the rest of
        // `self` and could not reach the field this uploads.
        let logo = self.logo_texture(root_ui.ctx());

        // The tab strip sits on its own surface, a shade off the panel below it and closed by a
        // line in the app's colour, so the chrome is visibly chrome.
        let bar = {
            let visuals = root_ui.visuals();
            egui::Frame::new()
                .fill(visuals.window_fill)
                .inner_margin(egui::Margin::symmetric(8, 6))
        };
        egui::Panel::top("tabs")
            .frame(bar)
            .show_separator_line(false)
            .show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    // One run fills the first two tabs, so the second is offered only once there
                    // is something behind it. Greyed rather than hidden: a tab that appears
                    // partway through would move the one beside it under the pointer.
                    let ready = self.state.detail_ready();
                    for (tab, label, enabled) in [
                        (Tab::Surplus, "Cost recovery", true),
                        (Tab::Detail, "Peak power detail", ready),
                        // Always offered. It reads its own report and answers on its own, so
                        // nothing on the other two tabs has to have happened first.
                        (Tab::Reimbursement, "Evolute reimbursement", true),
                        // Last, and likewise always offered. It produces nothing the rest of the
                        // app reads, so it is where someone goes on purpose rather than on the
                        // way to something else.
                        (Tab::Convert, "Convert to workbook", true),
                    ] {
                        let selected = self.state.tab == tab;
                        let text = egui::RichText::new(label).bold();
                        let text = if selected {
                            text.color(theme::accent(ui))
                        } else {
                            text
                        };
                        if ui
                            .add_enabled(enabled, egui::Button::selectable(selected, text))
                            .clicked()
                        {
                            self.state.tab = tab;
                        }
                    }
                    // Pushed to the far end, away from the tabs: About is not a third place to
                    // work, and reads as chrome only if it does not sit in their row.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // The mark takes the corner and About sits inboard of it. The other way
                        // round would end the row with something that looks clickable and is not.
                        ui.add(
                            egui::Image::new(&logo)
                                .fit_to_exact_size(egui::vec2(LOGO_SIDE, LOGO_SIDE)),
                        );
                        if ui.button("About").clicked() {
                            self.about_open = true;
                        }
                        // Added last, so it sits inboard of About and the corner stays the
                        // mark's. It changes how the window looks and nothing about the work,
                        // which is what puts it in this group rather than in the tabs.
                        theme::toggle_button(ui);
                    });
                });
                let rect = ui.max_rect();
                let y = rect.bottom() + 6.0;
                ui.painter()
                    .hline(rect.x_range(), y, egui::Stroke::new(2.0, theme::accent(ui)));
            });

        // A run cleared by a changed input takes the detail tab's contents with it, so the tab it
        // is showing has to give way too.
        if self.state.tab == Tab::Detail && !self.state.detail_ready() {
            self.state.tab = Tab::Surplus;
        }

        egui::CentralPanel::default().show(root_ui, |ui| {
            // One scroll area for the whole tab: a long report scrolls together with the controls
            // that produced it, rather than being clipped into a panel of its own.
            //
            // Salted by the tab, so each keeps its own offset. Unsalted, the four share one, and a
            // tab opened while another was scrolled down opens part-way through its own contents —
            // which is what the detail tab did, arriving in the middle of the first interval.
            egui::ScrollArea::vertical()
                .id_salt(self.state.tab)
                .auto_shrink([false, false])
                .show(ui, |ui| match self.state.tab {
                    Tab::Surplus => {
                        surplus::ui(ui, &mut self.state.surplus, &mut self.state.working_dir)
                    }
                    Tab::Detail => {
                        detail::ui(ui, &mut self.state.surplus, &mut self.state.working_dir)
                    }
                    Tab::Reimbursement => reimbursement::ui(
                        ui,
                        &mut self.state.reimbursement,
                        &mut self.state.working_dir,
                    ),
                    Tab::Convert => {
                        convert::ui(ui, &mut self.state.convert, &mut self.state.working_dir)
                    }
                });
        });

        // After the panels, so it is drawn over whichever tab is open.
        about::window(root_ui.ctx(), &mut self.about_open);
    }
}
