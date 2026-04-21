//! Global hotkey listener module.
//!
//! Detects the global hotkey `Ctrl+Alt+Shift+Space` using the `rdev` crate
//! and sends a signal via channel when triggered.

use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// Starts the global hotkey listener.
///
/// Listens for `Ctrl+Alt+Shift+Space` combination and sends a signal
/// via the provided channel when the hotkey is pressed.
///
/// # Arguments
/// * `tx` - The sender channel to notify when hotkey is triggered
///
/// # Returns
/// * `anyhow::Result<()>` - Ok on success, error if listener fails to start
pub fn start_hotkey_listener(tx: Sender<()>) -> anyhow::Result<()> {
    // Track modifier key states using a Mutex for thread safety
    let state = Mutex::new(ModifierState::default());

    std::thread::spawn(move || {
        let callback = move |event: Event| {
            let mut state = state.lock().unwrap();

            match event.event_type {
                EventType::KeyPress(key) => match key {
                    Key::ControlLeft | Key::ControlRight => state.ctrl = true,
                    Key::Alt => state.alt = true,
                    Key::ShiftLeft | Key::ShiftRight => state.shift = true,
                    Key::Space => {
                        if state.ctrl && state.alt && state.shift {
                            let _ = tx.send(());
                        }
                    }
                    _ => {}
                },
                EventType::KeyRelease(key) => match key {
                    Key::ControlLeft | Key::ControlRight => state.ctrl = false,
                    Key::Alt => state.alt = false,
                    Key::ShiftLeft | Key::ShiftRight => state.shift = false,
                    _ => {}
                },
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            tracing::error!("Hotkey listener error: {:?}", e);
        }
    });

    tracing::info!("Hotkey listener started: Ctrl+Alt+Shift+Space");
    Ok(())
}

/// Tracks the state of modifier keys.
#[derive(Default)]
struct ModifierState {
    /// Whether Ctrl key is pressed
    ctrl: bool,
    /// Whether Alt key is pressed
    alt: bool,
    /// Whether Shift key is pressed
    shift: bool,
}
