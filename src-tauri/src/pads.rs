//! Controllers, through SDL3 - the same library and drivers the games use - as a small bridge that turns a gamepad
//! into the same moves a keyboard makes.
//!
//! The window gets buttons as *positions* (south, east, north, west) and the stick as a direction, so it can label
//! the hints the way the connected pad does (Nintendo's bottom button is B, Xbox's is A). SDL reads each pad with its
//! own driver (a Switch Pro Controller through HIDAPI, in whatever report mode a game left it), where gilrs, before,
//! read a Pro Controller in its full mode as a counter running through the buttons.
//!
//! While a game runs the manager lets go of the controllers (the gamepad subsystem is shut): the game's SDL owns them,
//! and two drivers configuring one Pro Controller at once would fight.
use crate::runner::Kind;
use crate::state::{PadStatus, Shared};
use sdl3_sys::events::{
    SDL_Event, SDL_WaitEventTimeout, SDL_EVENT_GAMEPAD_ADDED, SDL_EVENT_GAMEPAD_AXIS_MOTION,
    SDL_EVENT_GAMEPAD_BUTTON_DOWN, SDL_EVENT_GAMEPAD_BUTTON_UP, SDL_EVENT_GAMEPAD_REMOVED,
};
use sdl3_sys::gamepad::{
    SDL_CloseGamepad, SDL_Gamepad, SDL_GamepadAxis, SDL_GamepadButton, SDL_GetGamepadName,
    SDL_OpenGamepad,
};
use sdl3_sys::hints::{SDL_SetHint, SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS};
use sdl3_sys::init::{SDL_InitSubSystem, SDL_QuitSubSystem, SDL_INIT_GAMEPAD};
use sdl3_sys::joystick::SDL_JoystickID;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CStr;
use std::time::{Duration, Instant};
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

/// The pads open now, by their SDL instance id, with their names (in the order they came).
struct Pads {
    open: Vec<(SDL_JoystickID, *mut SDL_Gamepad, String)>,
}

impl Pads {
    fn status(&self, error: Option<String>) -> PadStatus {
        PadStatus {
            connected: !self.open.is_empty(),
            name: self.open.first().map(|p| p.2.clone()),
            error,
        }
    }

    fn close_all(&mut self) {
        for (_, g, _) in self.open.drain(..) {
            unsafe { SDL_CloseGamepad(g) };
        }
    }
}

fn sdl_error() -> String {
    unsafe { CStr::from_ptr(sdl3_sys::error::SDL_GetError()) }
        .to_string_lossy()
        .into_owned()
}

/// A game is running (it owns the controllers then)
fn game_running(shared: &Shared) -> bool {
    shared
        .runs
        .lock()
        .unwrap()
        .values()
        .any(|h| h.run.lock().map(|r| r.kind == Kind::Game).unwrap_or(false))
}

pub fn start(app: AppHandle, shared: Shared) {
    std::thread::spawn(move || {
        // (pad events reach the manager when its window is not in front too: the window itself ignores them then)
        unsafe { SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr()) };
        let mut pads = Pads { open: Vec::new() };
        let mut up = false; // (the gamepad subsystem is running)
        let mut last_dir = String::new();
        let (mut lx, mut ly) = (0.0f32, 0.0f32);
        let (mut lt, mut rt) = (false, false);
        // An axis counts only once it has been seen at rest (a stick that drifts or rests off centre must not drive
        // the window by itself), and a button that changes faster than a hand can press is not a button.
        let mut rest = Rest::default();
        let mut chatter = Chatter::default();
        loop {
            // ---- the controllers are the game's while one runs
            let paused = game_running(&shared);
            if paused && up {
                tracing::info!(target: "pads", "a game is running: the controllers are its own until it ends");
                pads.close_all();
                unsafe { SDL_QuitSubSystem(SDL_INIT_GAMEPAD) };
                up = false;
            }
            if paused {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            if !up {
                if !unsafe { SDL_InitSubSystem(SDL_INIT_GAMEPAD) } {
                    let e = sdl_error();
                    tracing::error!(target: "pads", error = %e, "gamepads could not start");
                    publish(&app, &shared, pads.status(Some(e)));
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                // (the pads already connected arrive as GAMEPAD_ADDED events)
                tracing::info!(target: "pads", "gamepads (SDL) started");
                up = true;
                last_dir.clear();
                (lx, ly, lt, rt) = (0.0, 0.0, false, false);
                rest = Rest::default();
                chatter = Chatter::default();
            }

            // ---- one event (pumping SDL), or a short wait
            let mut ev: SDL_Event = unsafe { std::mem::zeroed() };
            if !unsafe { SDL_WaitEventTimeout(&mut ev, 250) } {
                continue;
            }
            let ty = unsafe { ev.r#type };
            if ty == SDL_EVENT_GAMEPAD_ADDED.0 {
                let which = unsafe { ev.gdevice.which };
                if pads.open.iter().any(|p| p.0 == which) {
                    continue;
                }
                let g = unsafe { SDL_OpenGamepad(which) };
                if g.is_null() {
                    tracing::warn!(target: "pads", error = %sdl_error(), "a controller could not be opened");
                    continue;
                }
                let name = unsafe {
                    let n = SDL_GetGamepadName(g);
                    if n.is_null() {
                        "Controller".to_string()
                    } else {
                        CStr::from_ptr(n).to_string_lossy().into_owned()
                    }
                };
                tracing::info!(target: "pads", name = %name, "controller connected");
                pads.open.push((which, g, name));
                publish(&app, &shared, pads.status(None));
            } else if ty == SDL_EVENT_GAMEPAD_REMOVED.0 {
                let which = unsafe { ev.gdevice.which };
                if let Some(i) = pads.open.iter().position(|p| p.0 == which) {
                    let (_, g, name) = pads.open.remove(i);
                    unsafe { SDL_CloseGamepad(g) };
                    tracing::info!(target: "pads", name = %name, "controller disconnected");
                    last_dir.clear();
                    (lx, ly) = (0.0, 0.0);
                    rest = Rest::default();
                    chatter = Chatter::default();
                    publish(&app, &shared, pads.status(None));
                }
            } else if ty == SDL_EVENT_GAMEPAD_BUTTON_DOWN.0 || ty == SDL_EVENT_GAMEPAD_BUTTON_UP.0 {
                let b = unsafe { ev.gbutton };
                let pressed = b.down;
                tracing::debug!(target: "pads", button = b.button, pressed, "button");
                let name = button_name(SDL_GamepadButton(b.button as i32));
                if chatter.noisy(b.button as u32) {
                    // (its first noisy change: the window is told it is up, then hears no more of it)
                    if !pressed && chatter.just_marked.take() == Some(b.button as u32) {
                        if let Some(name) = name {
                            emit_button(&app, name, false);
                        }
                    }
                    continue;
                }
                if let Some(name) = name {
                    emit_button(&app, name, pressed);
                }
            } else if ty == SDL_EVENT_GAMEPAD_AXIS_MOTION.0 {
                let a = unsafe { ev.gaxis };
                let v = a.value as f32 / 32767.0;
                tracing::trace!(target: "pads", axis = a.axis, value = v, "axis");
                match SDL_GamepadAxis(a.axis as i32) {
                    // (SDL: y grows downwards)
                    SDL_GamepadAxis::LEFTX => {
                        lx = if rest.see(0, v.abs() < 0.25) { v } else { 0.0 };
                        stick(&app, lx, ly, &mut last_dir);
                    }
                    SDL_GamepadAxis::LEFTY => {
                        ly = if rest.see(1, v.abs() < 0.25) { v } else { 0.0 };
                        stick(&app, lx, ly, &mut last_dir);
                    }
                    SDL_GamepadAxis::LEFT_TRIGGER if rest.see(2, v < 0.2) => {
                        set_trigger(&app, "left", v > 0.45, &mut lt)
                    }
                    SDL_GamepadAxis::RIGHT_TRIGGER if rest.see(3, v < 0.2) => {
                        set_trigger(&app, "right", v > 0.45, &mut rt)
                    }
                    _ => {}
                }
            }
        }
    });
}

fn emit_button(app: &AppHandle, name: &str, pressed: bool) {
    let _ = app.emit(
        "pad-button",
        PadButton {
            button: name.into(),
            pressed,
        },
    );
}

/// Buttons that change state impossibly fast (4 changes within 120 ms): ignored until the pad reconnects.
#[derive(Default)]
struct Chatter {
    changes: HashMap<u32, VecDeque<Instant>>,
    ignored: HashSet<u32>,
    /// (the button marked noisy by the last call, so its release can still reach the window once)
    just_marked: Option<u32>,
}

impl Chatter {
    fn noisy(&mut self, code: u32) -> bool {
        if self.ignored.contains(&code) {
            return true;
        }
        let now = Instant::now();
        let q = self.changes.entry(code).or_default();
        q.push_back(now);
        while q
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_millis(120))
        {
            q.pop_front();
        }
        if q.len() >= 4 {
            self.ignored.insert(code);
            self.just_marked = Some(code);
            tracing::warn!(target: "pads", code,
                "a button changes faster than it can be pressed: ignored until the controller reconnects");
            return true;
        }
        false
    }
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

fn publish(app: &AppHandle, shared: &Shared, s: PadStatus) {
    *shared.pad.lock().unwrap() = s.clone();
    let _ = app.emit("pad-status", s);
}

/// SDL's positional buttons as the window names them (triggers come as axes: pad-trigger)
fn button_name(b: SDL_GamepadButton) -> Option<&'static str> {
    Some(match b {
        SDL_GamepadButton::SOUTH => "south",
        SDL_GamepadButton::EAST => "east",
        SDL_GamepadButton::NORTH => "north",
        SDL_GamepadButton::WEST => "west",
        SDL_GamepadButton::DPAD_UP => "up",
        SDL_GamepadButton::DPAD_DOWN => "down",
        SDL_GamepadButton::DPAD_LEFT => "left",
        SDL_GamepadButton::DPAD_RIGHT => "right",
        SDL_GamepadButton::LEFT_SHOULDER => "lb",
        SDL_GamepadButton::RIGHT_SHOULDER => "rb",
        SDL_GamepadButton::START => "start",
        SDL_GamepadButton::BACK => "select",
        SDL_GamepadButton::GUIDE => "mode",
        SDL_GamepadButton::LEFT_STICK => "l3",
        SDL_GamepadButton::RIGHT_STICK => "r3",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counting_button_is_ignored_a_pressed_one_is_not() {
        let mut c = Chatter::default();
        // a hand: press, release, press, release - far apart
        for _ in 0..2 {
            assert!(!c.noisy(1));
            std::thread::sleep(Duration::from_millis(70));
            assert!(!c.noisy(1));
            std::thread::sleep(Duration::from_millis(70));
        }
        // a counter bit: four changes in a few milliseconds
        assert!(!c.noisy(2));
        assert!(!c.noisy(2));
        assert!(!c.noisy(2));
        assert!(c.noisy(2));
        assert!(c.noisy(2), "stays ignored");
        assert!(!c.noisy(1), "the other button still counts");
    }

    #[test]
    fn sdl_positions_name_the_window_buttons() {
        assert_eq!(button_name(SDL_GamepadButton::SOUTH), Some("south"));
        assert_eq!(button_name(SDL_GamepadButton::LEFT_SHOULDER), Some("lb"));
        assert_eq!(button_name(SDL_GamepadButton::BACK), Some("select"));
        assert_eq!(button_name(SDL_GamepadButton::MISC1), None);
    }
}
