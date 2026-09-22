//! Touchpad scrolling with acceleration, for the one platform that needs it.
//!
//! winit reports a touchpad differently on each platform, and egui scales each report by its unit.
//! Windows and X11 deliver lines, which egui multiplies by 40 points apiece. A Wayland touchpad
//! delivers pixels, which egui takes as they stand, so the content moves exactly as far as the
//! fingers did: far slower than on Windows, and with no response to how fast the fingers move.
//! GTK and Qt apply their own gain on Wayland; egui applies none, and has no setting for it —
//! `line_scroll_speed` scales lines only.
//!
//! So point-unit wheel events are rescaled here, before egui sees them. Nothing else produces that
//! unit on the platforms this app ships for: a mouse wheel on Wayland still arrives as discrete
//! lines, and Windows never reports pixels. A mouse, and every Windows input, pass through as they
//! came.
//!
//! No inertia: the content stops when the fingers lift.

use eframe::egui::{Event, MouseWheelUnit, RawInput};

/// Points of content scrolled per physical pixel of finger travel, for the slowest movement.
///
/// 2.7 is what X11 gives: libinput there turns about 15 units of travel into one wheel click, and
/// egui scrolls 40 points a click.
const BASE_GAIN: f32 = 2.7;

/// The gain added for each physical pixel a single event carries, which is what makes a faster
/// swipe travel disproportionately further.
const ACCELERATION: f32 = 0.05;

/// The most any event is multiplied by, however fast the swipe.
const MAX_GAIN: f32 = 10.0;

/// Rescales every point-unit wheel event in `raw_input` to [`BASE_GAIN`] points per physical pixel
/// of finger travel, plus a gain that grows with the event's length. `pixels_per_point` is the
/// context's, so the result is the same at any display scale and any zoom. Every other event is
/// left as it is.
pub fn accelerate(raw_input: &mut RawInput, pixels_per_point: f32) {
    for event in &mut raw_input.events {
        if let Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta,
            ..
        } = event
        {
            // egui-winit has already divided the compositor's pixels by pixels-per-point, so the
            // finger's travel shrinks with the display scale and again with the app's zoom.
            // Multiplying back recovers it, which is what X11's line-based reports never lost.
            let physical = *delta * pixels_per_point;
            *delta = physical * gain(physical.length());
        }
    }
}

/// The multiplier for one event whose finger travel is `distance` physical pixels.
fn gain(distance: f32) -> f32 {
    // A touchpad reports at a steady rate while the fingers move, so the distance one event
    // carries is the swipe's speed. Scaling by it is what acceleration means here.
    (BASE_GAIN + ACCELERATION * distance).min(MAX_GAIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Modifiers, TouchPhase, Vec2, vec2};

    fn wheel(unit: MouseWheelUnit, delta: Vec2) -> Event {
        Event::MouseWheel {
            unit,
            delta,
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
        }
    }

    /// The delta of the single wheel event `accelerate` leaves behind, at one pixel per point.
    fn accelerated(unit: MouseWheelUnit, delta: Vec2) -> Vec2 {
        accelerated_at(unit, delta, 1.0)
    }

    /// The delta of the single wheel event `accelerate` leaves behind, at `pixels_per_point`.
    fn accelerated_at(unit: MouseWheelUnit, delta: Vec2, pixels_per_point: f32) -> Vec2 {
        let mut raw_input = RawInput {
            events: vec![wheel(unit, delta)],
            ..Default::default()
        };
        accelerate(&mut raw_input, pixels_per_point);
        match raw_input.events[..] {
            [Event::MouseWheel { delta, .. }] => delta,
            _ => panic!(
                "expected exactly one wheel event, got {:?}",
                raw_input.events
            ),
        }
    }

    #[test]
    fn a_mouse_wheel_is_left_alone() {
        let delta = vec2(0.0, -3.0);
        assert_eq!(accelerated(MouseWheelUnit::Line, delta), delta);
        assert_eq!(accelerated(MouseWheelUnit::Page, delta), delta);
    }

    #[test]
    fn the_slowest_touchpad_movement_gets_the_base_gain() {
        let delta = vec2(0.0, -0.01);
        let out = accelerated(MouseWheelUnit::Point, delta);
        assert!(
            (out.y / delta.y - BASE_GAIN).abs() < 0.01,
            "gain {}",
            out.y / delta.y
        );
    }

    #[test]
    fn a_faster_swipe_is_multiplied_by_more() {
        let slow = vec2(0.0, 2.0);
        let fast = vec2(0.0, 20.0);
        let slow_gain = accelerated(MouseWheelUnit::Point, slow).y / slow.y;
        let fast_gain = accelerated(MouseWheelUnit::Point, fast).y / fast.y;
        assert!(
            fast_gain > slow_gain,
            "{fast_gain} is not above {slow_gain}"
        );
    }

    #[test]
    fn the_gain_stops_at_its_ceiling() {
        let delta = vec2(0.0, 1000.0);
        assert_eq!(accelerated(MouseWheelUnit::Point, delta), delta * MAX_GAIN);
    }

    #[test]
    fn direction_is_kept_on_both_axes() {
        let out = accelerated(MouseWheelUnit::Point, vec2(-4.0, 3.0));
        assert!(out.x < 0.0 && out.y > 0.0, "{out:?}");
        // The whole vector is scaled by one gain, so a diagonal swipe keeps its angle.
        assert!((out.x / out.y - -4.0 / 3.0).abs() < 1e-5, "{out:?}");
    }

    #[test]
    fn every_other_event_passes_through() {
        let mut raw_input = RawInput {
            events: vec![Event::Text("x".into()), Event::Copy],
            ..Default::default()
        };
        accelerate(&mut raw_input, 1.0);
        assert_eq!(raw_input.events, vec![Event::Text("x".into()), Event::Copy]);
    }

    /// egui-winit hands over the finger's travel divided by pixels-per-point, so at twice the
    /// scale the same swipe arrives half as long. It must still scroll the same number of points.
    #[test]
    fn the_same_swipe_scrolls_as_far_at_any_scale() {
        let physical = vec2(0.0, 12.0);
        let at_one = accelerated_at(MouseWheelUnit::Point, physical, 1.0);
        let at_two_and_a_half = accelerated_at(MouseWheelUnit::Point, physical / 2.5, 2.5);
        assert!(
            (at_one - at_two_and_a_half).length() < 1e-4,
            "{at_one:?} at 1.0, {at_two_and_a_half:?} at 2.5"
        );
    }

    /// A pixel-unit delta left untouched would scroll at one point per pixel; the rescaled one
    /// scrolls at least [`BASE_GAIN`] times further.
    #[test]
    fn a_touchpad_scrolls_at_least_the_base_gain_per_pixel() {
        let physical = vec2(0.0, 5.0);
        let pixels_per_point = 1.3;
        let out = accelerated_at(
            MouseWheelUnit::Point,
            physical / pixels_per_point,
            pixels_per_point,
        );
        assert!(
            out.y >= physical.y * BASE_GAIN,
            "{} points for 5 pixels",
            out.y
        );
    }
}
