//! Visuals for both themes.
//!
//! Copied from `ev_peak_gui`; see `widgets.rs` for why. Kept close so the two apps look alike.
//!
//! egui's defaults are deliberately low-contrast: in the dark theme ordinary text is `gray(140)`
//! against a `gray(27)` panel, buttons carry no outline at all, and secondary text is dimmed to 60%
//! on top of that. The result reads as a wash of grey. What follows keeps egui's shape and spacing
//! and changes only what separates one thing from another:
//!
//! - **Text is brighter.** Near-white on dark, near-black on light, rather than mid-grey on either.
//! - **Widgets are outlined.** Every button, dropdown and field gets a visible border, so a control
//!   is distinguishable from the panel behind it without having to be hovered first.
//! - **The panel is pushed away from the widgets.** The window background goes darker (or lighter)
//!   while widget fills stay where they are, which widens the gap between the two.
//! - **Secondary text stays secondary but stays legible**, at 85% on dark and 90% on light rather
//!   than 60%.
//! - **The light theme is taken further than the dark one**, in every separation above and in the
//!   accent colour. A bright screen leaves the eye less range to spend, so a gap that reads clearly
//!   on a dark panel closes up on a light one. The two are not mirror images and should not be
//!   made into them.

use eframe::egui::{
    Color32, Context, FontData, FontDefinitions, FontFamily, RichText, Stroke, Theme, Ui, Visuals,
    style::WidgetVisuals,
};
use std::sync::Arc;

/// The family the bold face is registered under.
///
/// A named family rather than a weight, because egui has no notion of weight: a family is a list of
/// faces, and asking for bold means asking for a different list.
const BOLD: &str = "bold";

/// Applies the faces and the visuals for both themes, so the app looks deliberate whichever one the
/// system is on.
pub fn apply(ctx: &Context) {
    fonts(ctx);
    ctx.style_mut_of(Theme::Dark, |style| dark(&mut style.visuals));
    ctx.style_mut_of(Theme::Light, |style| light(&mut style.visuals));
}

/// Installs Inter in place of egui's default proportional face, and registers its bold weight.
///
/// egui bundles Ubuntu-Light and no bold face at all, which is why [`RichText::strong`] changes the
/// colour and not the weight. A light-weight face on a pale panel reads as grey however black the
/// colour actually is, so the light theme's washed-out look is a matter of the face rather than of
/// the greys — darkening those was treating the symptom.
///
/// Inter is drawn for user interfaces at small sizes: a tall x-height and open letterforms, which
/// is what holds 13pt text together on a bright screen. It is under the SIL Open Font License, and
/// both faces are embedded in the binary, so the app renders identically on a machine that has
/// never heard of it.
///
/// Both are added at the *front* of their family's list rather than replacing it. egui's own faces
/// stay behind them and still serve the glyphs Inter does not carry — the theme toggle's sun and
/// moon among them.
fn fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    for (name, bytes) in [
        (
            "Inter",
            include_bytes!("../../../assets/fonts/Inter-Regular.ttf").as_slice(),
        ),
        (
            "Inter-Bold",
            include_bytes!("../../../assets/fonts/Inter-Bold.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "Inter".to_owned());

    // The bold family is the proportional one with the bold face in front, so anything Inter-Bold
    // lacks falls back through exactly the same chain as ordinary text.
    let mut bold = proportional.clone();
    bold.insert(0, "Inter-Bold".to_owned());
    fonts.families.insert(FontFamily::Name(BOLD.into()), bold);

    ctx.set_fonts(fonts);
}

/// Text in the bold face.
///
/// An extension rather than a call to [`RichText::strong`], which does not do this: egui's `strong`
/// only swaps in a stronger colour, and in the light theme that colour is the same black ordinary
/// text is already drawn in — so every `strong` in this app used to be invisible on a light screen.
/// Calling `strong` as well is deliberate: on the dark theme the colour lift still helps, and this
/// is the one method either theme needs.
pub trait Bold {
    /// This text, in the bold face.
    fn bold(self) -> Self;
}

impl Bold for RichText {
    fn bold(self) -> Self {
        self.strong().family(FontFamily::Name(BOLD.into()))
    }
}

/// A button that switches between the two themes, named for the one it switches *to*.
///
/// Named that way because it is the only reading that does not have to be guessed at: a button
/// labelled "Dark" while the window is already dark says nothing about what pressing it does.
///
/// The choice lasts as long as the window. Nothing is written to disk, so the next start follows
/// the system setting again — which is what a user who changes their whole desktop to dark expects,
/// and the reason this is a toggle rather than a setting.
pub fn toggle_button(ui: &mut Ui) {
    let (label, hover, target) = if ui.visuals().dark_mode {
        ("☀ Light", "Switch to the light theme", Theme::Light)
    } else {
        ("🌙 Dark", "Switch to the dark theme", Theme::Dark)
    };
    if ui.button(label).on_hover_text(hover).clicked() {
        // Takes effect on the next frame, and applies to every window the context draws, so the
        // About window changes with the panels behind it.
        ui.ctx().set_theme(target);
    }
}

/// The app's colour, taken from the icon: a teal that carries headings, the selected tab and the
/// lower of the two estimates. One accent, used in few places, is what keeps it from looking busy.
///
/// The two are not the same teal at different lightnesses. Each is chosen against the panel it is
/// drawn on: the dark one has only to be bright enough, while the light one is a heading in colour
/// on a pale ground, which is the hardest thing on either screen to keep legible. It is therefore
/// darker than a mirror image of the dark one would be.
pub fn accent(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(52, 200, 165)
    } else {
        Color32::from_rgb(6, 86, 70)
    }
}

/// Outline, fill and text for one widget state, in one line at each call site.
fn widget(fill: u8, stroke: u8, text: u8, stroke_width: f32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: Color32::from_gray(fill),
        weak_bg_fill: Color32::from_gray(fill),
        bg_stroke: Stroke::new(stroke_width, Color32::from_gray(stroke)),
        fg_stroke: Stroke::new(1.0, Color32::from_gray(text)),
        corner_radius: 3.into(),
        expansion: 0.0,
    }
}

fn dark(v: &mut Visuals) {
    // The window recedes; the widgets keep their fill, so the two no longer read as one surface.
    v.panel_fill = Color32::from_gray(16);
    v.window_fill = Color32::from_gray(32);
    v.extreme_bg_color = Color32::from_gray(8);
    v.faint_bg_color = Color32::from_gray(40);
    v.code_bg_color = Color32::from_gray(44);
    v.window_stroke = Stroke::new(1.0, Color32::from_gray(115));

    //                       fill  outline text  outline width
    v.widgets.noninteractive = widget(16, 105, 236, 1.0); // labels, separators, indent lines
    v.widgets.inactive = widget(66, 130, 245, 1.0); // a button at rest — now outlined
    v.widgets.hovered = widget(88, 195, 255, 1.5);
    v.widgets.active = widget(110, 255, 255, 1.5);
    v.widgets.open = widget(76, 150, 248, 1.0);

    v.selection.bg_fill = Color32::from_rgb(14, 105, 90);
    v.selection.stroke = Stroke::new(1.0, Color32::from_gray(255));

    // Pure red on a dark panel is hard to read; this stays unmistakably an error and legible.
    v.error_fg_color = Color32::from_rgb(255, 105, 95);
    v.warn_fg_color = Color32::from_rgb(255, 175, 60);
    v.hyperlink_color = Color32::from_rgb(52, 200, 165);

    // Secondary text — the explanatory notes under headings — should read as secondary without
    // becoming unreadable, which is what egui's 60% does over a low-contrast base.
    v.weak_text_alpha = 0.85;

    common(v);
}

fn light(v: &mut Visuals) {
    // The same idea as the dark theme, not its mirror image: the panel takes a light grey so that
    // white widgets stand out from it.
    //
    // Every separation here is wider than the dark theme's counterpart, and the accent is darker.
    // A bright ground leaves the eye less range to spend, so greys that look distinct against a
    // dark panel run together against a pale one. Reflecting the dark theme's numbers through the
    // middle is what produced the wash of grey this replaces.
    v.panel_fill = Color32::from_gray(216);
    v.window_fill = Color32::from_gray(252);
    v.extreme_bg_color = Color32::from_gray(255);
    v.faint_bg_color = Color32::from_gray(232);
    v.code_bg_color = Color32::from_gray(229);
    v.window_stroke = Stroke::new(1.0, Color32::from_gray(90));

    //                       fill  outline text  outline width
    v.widgets.noninteractive = widget(216, 105, 0, 1.0);
    v.widgets.inactive = widget(253, 85, 0, 1.0);
    v.widgets.hovered = widget(255, 45, 0, 1.5);
    v.widgets.active = widget(206, 0, 0, 1.5);
    v.widgets.open = widget(247, 75, 0, 1.0);

    v.selection.bg_fill = Color32::from_rgb(168, 222, 208);
    v.selection.stroke = Stroke::new(1.0, Color32::from_gray(0));

    v.error_fg_color = Color32::from_rgb(180, 0, 0);
    v.warn_fg_color = Color32::from_rgb(130, 74, 0);
    // The accent again. A link is the one other place the app's colour carries meaning, and two
    // teals a shade apart would read as a mistake rather than as a distinction.
    v.hyperlink_color = Color32::from_rgb(6, 86, 70);

    // Higher than the dark theme's, for the reason the whole of this function is: dimmed text on a
    // pale ground loses more than the same dimming loses on a dark one.
    v.weak_text_alpha = 0.90;

    common(v);
}

/// What both themes set the same way. Secondary text is not here: it is the one setting whose
/// right value differs between them — see each function's own `weak_text_alpha`, and the module
/// doc for why the light theme needs more of everything.
fn common(v: &mut Visuals) {
    // A striped table is easier to follow than an unstriped one, and the stripe is now visible.
    v.striped = true;
    v.button_frame = true;
    v.collapsing_header_frame = true;
    v.indent_has_left_vline = true;
}
