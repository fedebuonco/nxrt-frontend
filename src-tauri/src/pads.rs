//! Controllers, through gilrs: a small bridge that turns a gamepad into the same moves a keyboard makes.
//!
//! The window gets buttons as *positions* (south, east, north, west) and the stick as a direction, so it can label
//! the hints the way the connected pad does (Nintendo's bottom button is B, Xbox's is A).
use crate::state::{PadStatus, Shared};
use gilrs::{Axis, Button, EventType, Gilrs};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct PadButton {
    button: String,
    pressed: bool,
}

#[derive(Serialize, Clone)]
struct PadDir {
    dir: String,
}

#[derive(Serialize, Clone)]
struct PadTrigger {
    side: String,
    pressed: bool,
}

pub fn start(app: AppHandle, shared: Shared) {
    std::thread::spawn(move || {
        let mut gilrs = match Gilrs::new() {
            Ok(g) => g,
            Err(e) => {
                tracing::error!(target: "pads", error = %e, "gamepads could not start");
                publish(
                    &app,
                    &shared,
                    PadStatus {
                        connected: false,
                        name: None,
                        error: Some(format!("{e}")),
                    },
                );
                return;
            }
        };
        let first = status(&gilrs);
        if first.connected {
            tracing::info!(target: "pads", name = ?first.name, "controller connected at start");
        } else {
            tracing::info!(target: "pads", "no controller at start");
        }
        publish(&app, &shared, first);
        let mut last_dir = String::new();
        let (mut lx, mut ly) = (0.0f32, 0.0f32);
        let (mut lt, mut rt) = (false, false);
        // An axis counts only once it has been seen at rest: a stick that drifts or rests off centre, or an axis that
        // rests at an end (some pads report one for a trigger, a gyro or nothing), must not drive the window by itself.
        let mut rest = Rest::default();
        loop {
            // (blocking: a GUI has no event loop of its own to spin)
            let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(500))) else {
                continue;
            };
            match ev.event {
                EventType::ButtonPressed(b, _) => {
                    if let Some(name) = button_name(b) {
                        let _ = app.emit(
                            "pad-button",
                            PadButton {
                                button: name.into(),
                                pressed: true,
                            },
                        );
                    }
                    // (some pads report their triggers only as buttons: keep the trigger state in step)
                    match b {
                        Button::LeftTrigger2 => set_trigger(&app, "left", true, &mut lt),
                        Button::RightTrigger2 => set_trigger(&app, "right", true, &mut rt),
                        _ => {}
                    }
                }
                EventType::ButtonReleased(b, _) => {
                    if let Some(name) = button_name(b) {
                        let _ = app.emit(
                            "pad-button",
                            PadButton {
                                button: name.into(),
                                pressed: false,
                            },
                        );
                    }
                    match b {
                        Button::LeftTrigger2 => set_trigger(&app, "left", false, &mut lt),
                        Button::RightTrigger2 => set_trigger(&app, "right", false, &mut rt),
                        _ => {}
                    }
                }
                EventType::ButtonRepeated(_, _) => {}
                EventType::AxisChanged(axis, value, _) => match axis {
                    Axis::LeftStickX => {
                        lx = if rest.see(0, value.abs() < 0.25) {
                            value
                        } else {
                            0.0
                        };
                        stick(&app, lx, ly, &mut last_dir);
                    }
                    Axis::LeftStickY => {
                        ly = if rest.see(1, value.abs() < 0.25) {
                            value
                        } else {
                            0.0
                        };
                        stick(&app, lx, ly, &mut last_dir);
                    }
                    Axis::LeftZ if rest.see(2, value < 0.2) => {
                        set_trigger(&app, "left", value > 0.45, &mut lt)
                    }
                    Axis::RightZ if rest.see(3, value < 0.2) => {
                        set_trigger(&app, "right", value > 0.45, &mut rt)
                    }
                    _ => {}
                },
                EventType::Connected | EventType::Disconnected => {
                    last_dir.clear();
                    lx = 0.0;
                    ly = 0.0;
                    rest = Rest::default();
                    let first = status(&gilrs);
                    if first.connected {
                        tracing::info!(target: "pads", name = ?first.name, "controller connected at start");
                    } else {
                        tracing::info!(target: "pads", "no controller at start");
                    }
                    publish(&app, &shared, first);
                }
                _ => {}
            }
        }
    });
}

/// Which axes (left stick x, y, left and right trigger) have been seen at rest since the pad connected.
#[derive(Default)]
struct Rest([bool; 4]);

impl Rest {
    /// Whether axis i may drive the window: once at rest, always (until the pad reconnects).
    fn see(&mut self, i: usize, at_rest: bool) -> bool {
        if at_rest && !self.0[i] {
            self.0[i] = true;
            tracing::debug!(target: "pads", axis = i, "axis at rest: it counts from now");
        }
        self.0[i]
    }
}

/// One direction at a time: the dominant stick axis past a deadzone. Emitted only when it changes.
fn stick(app: &AppHandle, x: f32, y: f32, last: &mut String) {
    const DEAD: f32 = 0.55;
    let dir = if x.abs() < DEAD && y.abs() < DEAD {
        ""
    } else if x.abs() > y.abs() {
        if x < 0.0 {
            "left"
        } else {
            "right"
        }
    } else if y < 0.0 {
        "up"
    } else {
        "down"
    };
    if dir != last.as_str() {
        *last = dir.to_string();
        tracing::debug!(target: "pads", dir, x, y, "stick direction");
        let _ = app.emit(
            "pad-dir",
            PadDir {
                dir: dir.to_string(),
            },
        );
    }
}

fn set_trigger(app: &AppHandle, side: &str, pressed: bool, state: &mut bool) {
    if *state != pressed {
        *state = pressed;
        tracing::debug!(target: "pads", side, pressed, "trigger");
        let _ = app.emit(
            "pad-trigger",
            PadTrigger {
                side: side.into(),
                pressed,
            },
        );
    }
}

fn status(g: &Gilrs) -> PadStatus {
    let name = g.gamepads().next().map(|(_, gp)| gp.name().to_string());
    PadStatus {
        connected: g.gamepads().count() > 0,
        name,
        error: None,
    }
}

fn publish(app: &AppHandle, shared: &Shared, s: PadStatus) {
    *shared.pad.lock().unwrap() = s.clone();
    let _ = app.emit("pad-status", s);
}

fn button_name(b: Button) -> Option<&'static str> {
    Some(match b {
        Button::South => "south",
        Button::East => "east",
        Button::North => "north",
        Button::West => "west",
        Button::DPadUp => "up",
        Button::DPadDown => "down",
        Button::DPadLeft => "left",
        Button::DPadRight => "right",
        Button::LeftTrigger => "lb",
        Button::RightTrigger => "rb",
        Button::LeftTrigger2 => "lt",
        Button::RightTrigger2 => "rt",
        Button::Start => "start",
        Button::Select => "select",
        Button::Mode => "mode",
        Button::LeftThumb => "l3",
        Button::RightThumb => "r3",
        _ => return None,
    })
}
