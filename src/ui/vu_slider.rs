//! Merged VU + Slider canvas widget — Wave Link 3.0's signature visual.
//!
//! The VU meter fill IS the slider track background: a green/amber/red bar
//! grows behind a draggable thumb. This replaces the separate `audio_slider`
//! + `vu_meter` widgets in matrix cells.

use iced::mouse;
use iced::widget::canvas::Action;
use iced::widget::canvas::{self, Cache, Event, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Font, Length, Pixels, Point, Rectangle, Size, Theme};

use crate::app::Message;
use crate::plugin::api::SourceId;
use crate::ui::theme::{ThemeMode, VU_AMBER, VU_GREEN, VU_RED, bg_hover, border_color, text_muted};

// --- Layout constants ---
const TRACK_HEIGHT: f32 = 24.0;
const TRACK_RADIUS: f32 = 4.0;
const THUMB_WIDTH: f32 = 6.0;
const THUMB_HEIGHT: f32 = 20.0;
const THUMB_RADIUS: f32 = 2.0;
const DB_LABEL_HEIGHT: f32 = 14.0;
const WIDGET_HEIGHT: f32 = TRACK_HEIGHT + DB_LABEL_HEIGHT;
const PAD_H: f32 = 3.0; // Horizontal padding for thumb travel

/// The canvas program for the merged VU+Slider.
pub struct VuSliderProgram {
    /// Current volume 0.0..=1.0
    pub volume: f32,
    /// Current peak level 0.0..=1.0 (drives VU fill)
    pub peak: f32,
    /// Whether the route is muted
    pub muted: bool,
    /// Source ID for message emission
    pub source: SourceId,
    /// Mix ID for message emission
    pub mix_id: u32,
    /// Theme mode for colors
    pub theme_mode: ThemeMode,
}

/// Per-widget mutable state — tracks drag interaction.
#[derive(Default)]
pub struct VuSliderState {
    cache: Cache,
    dragging: bool,
}

/// Build the merged VU+Slider element.
pub fn vu_slider<'a>(
    volume: f32,
    peak: f32,
    muted: bool,
    source: SourceId,
    mix_id: u32,
    theme_mode: ThemeMode,
) -> Element<'a, Message> {
    let program = VuSliderProgram {
        volume: volume.clamp(0.0, 1.0),
        peak: peak.clamp(0.0, 1.0),
        muted,
        source,
        mix_id,
        theme_mode,
    };

    tracing::trace!(
        volume,
        peak,
        muted,
        source = ?source,
        mix_id,
        "vu_slider rendered"
    );

    canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fixed(WIDGET_HEIGHT))
        .into()
}

// --- Helpers ---

/// Usable track width given total bounds width.
fn track_width(bounds_w: f32) -> f32 {
    (bounds_w - 2.0 * PAD_H).max(1.0)
}

/// Convert a 0..1 value to an X pixel position within the track.
fn value_to_x(value: f32, bounds_w: f32) -> f32 {
    PAD_H + value.clamp(0.0, 1.0) * track_width(bounds_w)
}

/// Convert an X pixel position to a 0..1 value.
fn x_to_value(x: f32, bounds_w: f32) -> f32 {
    ((x - PAD_H) / track_width(bounds_w)).clamp(0.0, 1.0)
}

/// VU fill color based on peak level.
fn vu_color(peak: f32) -> Color {
    if peak < 0.70 {
        VU_GREEN
    } else if peak < 0.90 {
        VU_AMBER
    } else {
        VU_RED
    }
}

/// Format volume as dB string.
fn volume_to_db(value: f32) -> String {
    if value <= 0.001 {
        "-inf dB".to_string()
    } else {
        format!("{:.1} dB", 20.0 * value.log10())
    }
}

impl canvas::Program<Message> for VuSliderProgram {
    type State = VuSliderState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    if pos.y <= TRACK_HEIGHT {
                        state.dragging = true;
                        state.cache.clear();
                        let new_val = x_to_value(pos.x, bounds.width);
                        return Some(
                            Action::publish(Message::RouteVolumeChanged {
                                source: self.source,
                                mix: self.mix_id,
                                volume: new_val,
                            })
                            .and_capture(),
                        );
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.dragging {
                    state.dragging = false;
                    state.cache.clear();
                    return Some(Action::capture());
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging {
                    if let Some(pos) = cursor.position_in(bounds) {
                        state.cache.clear();
                        let new_val = x_to_value(pos.x, bounds.width);
                        return Some(
                            Action::publish(Message::RouteVolumeChanged {
                                source: self.source,
                                mix: self.mix_id,
                                volume: new_val,
                            })
                            .and_capture(),
                        );
                    }
                }
            }
            _ => {}
        }

        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            self.draw_track(frame, bounds.size());
            self.draw_vu_fill(frame, bounds.size());
            self.draw_thumb(frame, bounds.size(), state.dragging);
            self.draw_db_label(frame, bounds.size());
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging {
            return mouse::Interaction::Grabbing;
        }
        if let Some(pos) = cursor.position_in(bounds) {
            if pos.y <= TRACK_HEIGHT {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

impl VuSliderProgram {
    /// Draw the track background (rounded rectangle).
    fn draw_track(&self, frame: &mut Frame, size: Size) {
        let bg = bg_hover(self.theme_mode);
        let track = rounded_rect(
            PAD_H,
            0.0,
            track_width(size.width),
            TRACK_HEIGHT,
            TRACK_RADIUS,
        );
        frame.fill(&track, bg);

        // Subtle border
        let border = border_color(self.theme_mode);
        frame.stroke(
            &rounded_rect(
                PAD_H,
                0.0,
                track_width(size.width),
                TRACK_HEIGHT,
                TRACK_RADIUS,
            ),
            Stroke {
                style: canvas::stroke::Style::Solid(Color { a: 0.5, ..border }),
                width: 1.0,
                ..Stroke::default()
            },
        );
    }

    /// Draw VU fill as the track background — the signature Wave Link visual.
    fn draw_vu_fill(&self, frame: &mut Frame, size: Size) {
        let peak = if self.muted { 0.0 } else { self.peak };
        if peak <= 0.001 {
            return; // No fill when silent
        }

        let fill_w = peak * track_width(size.width);
        let color = vu_color(peak);
        // Use alpha for a softer, backlit look
        let fill_color = Color { a: 0.6, ..color };

        // Clip to track bounds with rounded left side
        let fill = rounded_rect_left(PAD_H, 0.0, fill_w, TRACK_HEIGHT, TRACK_RADIUS);
        frame.fill(&fill, fill_color);
    }

    /// Draw the thumb at the current volume position.
    fn draw_thumb(&self, frame: &mut Frame, size: Size, dragging: bool) {
        let x = value_to_x(self.volume, size.width);
        let thumb_x = x - THUMB_WIDTH / 2.0;
        let thumb_y = (TRACK_HEIGHT - THUMB_HEIGHT) / 2.0;

        // Thumb body
        let brightness = if dragging { 1.0 } else { 0.88 };
        let thumb_color = Color {
            r: brightness,
            g: brightness,
            b: brightness,
            a: 1.0,
        };
        let thumb = rounded_rect(thumb_x, thumb_y, THUMB_WIDTH, THUMB_HEIGHT, THUMB_RADIUS);
        frame.fill(&thumb, thumb_color);

        // Thumb shadow/border
        let shadow_color = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.3,
        };
        frame.stroke(
            &rounded_rect(thumb_x, thumb_y, THUMB_WIDTH, THUMB_HEIGHT, THUMB_RADIUS),
            Stroke {
                style: canvas::stroke::Style::Solid(shadow_color),
                width: 1.0,
                ..Stroke::default()
            },
        );
    }

    /// Draw the dB readout below the track.
    fn draw_db_label(&self, frame: &mut Frame, size: Size) {
        let db_text = volume_to_db(self.volume);
        let label_color = text_muted(self.theme_mode);

        frame.fill_text(Text {
            content: db_text,
            position: Point::new(size.width / 2.0, TRACK_HEIGHT + 2.0),
            color: label_color,
            size: Pixels(10.0),
            font: Font::default(),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Top,
            ..Text::default()
        });
    }
}

// --- Path helpers ---

/// Rounded rectangle path.
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    Path::new(|b| {
        let r = r.min(w / 2.0).min(h / 2.0);
        b.move_to(Point::new(x + r, y));
        b.line_to(Point::new(x + w - r, y));
        b.arc_to(Point::new(x + w, y), Point::new(x + w, y + r), r);
        b.line_to(Point::new(x + w, y + h - r));
        b.arc_to(Point::new(x + w, y + h), Point::new(x + w - r, y + h), r);
        b.line_to(Point::new(x + r, y + h));
        b.arc_to(Point::new(x, y + h), Point::new(x, y + h - r), r);
        b.line_to(Point::new(x, y + r));
        b.arc_to(Point::new(x, y), Point::new(x + r, y), r);
        b.close();
    })
}

/// Left-side rounded rectangle (for VU fill that clips at current level).
fn rounded_rect_left(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    Path::new(|b| {
        let r = r.min(w / 2.0).min(h / 2.0);
        b.move_to(Point::new(x + r, y));
        b.line_to(Point::new(x + w, y)); // Flat right edge
        b.line_to(Point::new(x + w, y + h)); // Flat right edge
        b.line_to(Point::new(x + r, y + h));
        b.arc_to(Point::new(x, y + h), Point::new(x, y + h - r), r);
        b.line_to(Point::new(x, y + r));
        b.arc_to(Point::new(x, y), Point::new(x + r, y), r);
        b.close();
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_to_x_bounds() {
        let w = 200.0;
        let x_min = value_to_x(0.0, w);
        let x_max = value_to_x(1.0, w);
        assert!(
            (x_min - PAD_H).abs() < 0.01,
            "x_min={x_min}, expected {PAD_H}"
        );
        assert!(
            (x_max - (w - PAD_H)).abs() < 0.01,
            "x_max={x_max}, expected {}",
            w - PAD_H
        );
    }

    #[test]
    fn test_x_to_value_roundtrip() {
        let w = 300.0;
        for val in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let x = value_to_x(val, w);
            let back = x_to_value(x, w);
            assert!(
                (back - val).abs() < 0.001,
                "roundtrip failed: {val} -> {x} -> {back}"
            );
        }
    }

    #[test]
    fn test_vu_color_zones() {
        assert_eq!(vu_color(0.0), VU_GREEN);
        assert_eq!(vu_color(0.5), VU_GREEN);
        assert_eq!(vu_color(0.75), VU_AMBER);
        assert_eq!(vu_color(0.95), VU_RED);
    }

    #[test]
    fn test_volume_to_db_inf() {
        assert_eq!(volume_to_db(0.0), "-inf dB");
        assert_eq!(volume_to_db(0.0005), "-inf dB");
    }

    #[test]
    fn test_volume_to_db_unity() {
        let db = volume_to_db(1.0);
        assert!(db.contains("0.0"), "expected ~0 dB, got {db}");
    }
}
