//! The fn (🌐) key alone as the dictation shortcut, on macOS.
//!
//! The global-shortcut plugin can't register a modifier on its own, and fn
//! least of all, so an event tap follows the key instead: macOS reports it as
//! a "flags changed" event for keycode 63 (`kVK_Function`), down while the
//! event's flags carry `kCGEventFlagMaskSecondaryFn`. Presses and releases go
//! to the dictation loop exactly like the plugin's, so every shortcut mode
//! behaves the same.
//!
//! The tap needs the Accessibility permission Parler already asks for. Until
//! it has it, the window says so and the tap is retried every few seconds,
//! so fn works as soon as the permission is granted.

use crate::dictation::{Dictation, Event};
use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// `kVK_Function`: the fn key, labelled 🌐 on recent keyboards.
const KEYCODE: i64 = 63;
/// `kCGEventFlagMaskSecondaryFn`.
const SECONDARY_FN: u64 = 0x0080_0000;
/// How often a tap that couldn't be created is tried again.
const RETRY: Duration = Duration::from_secs(2);
/// How often the tap's thread checks whether it should stop.
const SLICE: Duration = Duration::from_millis(500);

/// What a keyboard event means for the fn shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Press,
    Release,
    /// Another key went down while fn was held: fn was a modifier (fn + ←,
    /// fn + Delete…), not a request to dictate.
    Combined,
}

/// Follows the fn key through the events of a tap.
#[derive(Debug, Default)]
pub struct Tracker {
    down: bool,
    combined: bool,
}

impl Tracker {
    /// A modifier changed. Only the fn key's own events count: arrows and the
    /// F keys carry the fn flag too.
    pub fn flags_changed(&mut self, keycode: i64, flags: u64) -> Option<Signal> {
        if keycode != KEYCODE {
            return None;
        }
        let down = flags & SECONDARY_FN != 0;
        if down == self.down {
            return None;
        }
        self.down = down;
        if down {
            self.combined = false;
            Some(Signal::Press)
        } else {
            Some(Signal::Release)
        }
    }

    /// A key went down.
    pub fn key_down(&mut self) -> Option<Signal> {
        if self.down && !self.combined {
            self.combined = true;
            Some(Signal::Combined)
        } else {
            None
        }
    }

    /// The tap was paused, so a release may have been missed: assume fn is
    /// up rather than leave a dictation running.
    pub fn reset(&mut self) -> Option<Signal> {
        std::mem::take(&mut self.down).then_some(Signal::Release)
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

struct Running {
    stop: Arc<AtomicBool>,
    run_loop: Arc<Mutex<Option<CFRunLoop>>>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Starts following the fn key, unless it already is.
pub fn start(app: &AppHandle) {
    let mut running = lock(&RUNNING);
    if running.is_some() {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let run_loop = Arc::new(Mutex::new(None));
    let spawned = thread::Builder::new().name("fn-key".into()).spawn({
        let app = app.clone();
        let stop = stop.clone();
        let run_loop = run_loop.clone();
        move || listen(app, stop, run_loop)
    });
    match spawned {
        Ok(_) => *running = Some(Running { stop, run_loop }),
        Err(e) => log::warn!("Could not start listening to the fn key: {e}"),
    }
}

/// Stops following the fn key; another shortcut takes over.
pub fn stop() {
    if let Some(running) = lock(&RUNNING).take() {
        running.stop.store(true, Ordering::SeqCst);
        if let Some(run_loop) = lock(&running.run_loop).as_ref() {
            run_loop.stop();
        }
    }
}

pub fn is_running() -> bool {
    lock(&RUNNING).is_some()
}

fn listen(app: AppHandle, stop: Arc<AtomicBool>, run_loop: Arc<Mutex<Option<CFRunLoop>>>) {
    let mut refused = false;
    let tap = loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        match create_tap(&app, &stop) {
            Ok(tap) => break tap,
            Err(()) => {
                if !refused {
                    refused = true;
                    log::warn!("The fn key needs Accessibility access; retrying until granted");
                    crate::shortcut::report_problem(
                        &app,
                        crate::shortcut::FN,
                        "Accessibility access is needed to listen to the fn key".to_string(),
                    );
                }
                thread::sleep(RETRY);
            }
        }
    };
    if refused {
        crate::shortcut::clear_problem(&app);
    }

    let Ok(source) = tap.mach_port().create_runloop_source(0) else {
        log::warn!("Could not listen to the fn key: no run loop source");
        return;
    };
    let current = CFRunLoop::get_current();
    current.add_source(&source, unsafe { kCFRunLoopCommonModes });
    tap.enable();
    *lock(&run_loop) = Some(current);
    log::info!("Listening to the fn key");

    // In slices, so a stop requested at any moment is seen.
    while !stop.load(Ordering::SeqCst) {
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, SLICE, false);
    }
    log::info!("Stopped listening to the fn key");
    // Dropping the tap removes it.
}

fn create_tap(app: &AppHandle, stop: &Arc<AtomicBool>) -> Result<CGEventTap<'static>, ()> {
    let tracker = RefCell::new(Tracker::default());
    // Filled in once the tap exists, so its callback can turn it back on.
    let port = Arc::new(AtomicPtr::<c_void>::new(std::ptr::null_mut()));
    let app = app.clone();
    let stop = stop.clone();
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        // An active tap is what Accessibility allows; events pass unchanged.
        CGEventTapOptions::Default,
        vec![CGEventType::FlagsChanged, CGEventType::KeyDown],
        {
            let port = port.clone();
            move |_proxy, kind, event| {
                if stop.load(Ordering::Relaxed) {
                    return CallbackResult::Keep;
                }
                let signal = match kind {
                    CGEventType::FlagsChanged => tracker.borrow_mut().flags_changed(
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
                        event.get_flags().bits(),
                    ),
                    CGEventType::KeyDown => tracker.borrow_mut().key_down(),
                    // macOS pauses a tap it finds slow, or on secure input.
                    CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                        let tap = port.load(Ordering::SeqCst);
                        if !tap.is_null() {
                            log::info!("The fn key listener was paused; resuming");
                            unsafe { CGEventTapEnable(tap as CFMachPortRef, true) };
                        }
                        tracker.borrow_mut().reset()
                    }
                    _ => None,
                };
                if let Some(signal) = signal {
                    app.state::<Dictation>().send(match signal {
                        Signal::Press => Event::Press,
                        Signal::Release => Event::Release,
                        Signal::Combined => Event::Interrupted,
                    });
                }
                CallbackResult::Keep
            }
        },
    )?;
    port.store(
        tap.mach_port().as_concrete_TypeRef() as *mut c_void,
        Ordering::SeqCst,
    );
    Ok(tap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: u64 = 0x0002_0000;

    #[test]
    fn fn_goes_down_and_up() {
        let mut fn_key = Tracker::default();
        assert_eq!(
            fn_key.flags_changed(KEYCODE, SECONDARY_FN),
            Some(Signal::Press)
        );
        // macOS may repeat the state: one press is one press.
        assert_eq!(fn_key.flags_changed(KEYCODE, SECONDARY_FN), None);
        assert_eq!(fn_key.flags_changed(KEYCODE, 0), Some(Signal::Release));
        assert_eq!(fn_key.flags_changed(KEYCODE, 0), None);
    }

    #[test]
    fn other_keys_with_the_fn_flag_are_ignored() {
        let mut fn_key = Tracker::default();
        // Shift (56) changing while an arrow sets the fn flag.
        assert_eq!(fn_key.flags_changed(56, SHIFT | SECONDARY_FN), None);
        assert_eq!(fn_key.flags_changed(56, 0), None);
        // fn with Shift already held still counts.
        assert_eq!(
            fn_key.flags_changed(KEYCODE, SHIFT | SECONDARY_FN),
            Some(Signal::Press)
        );
        assert_eq!(fn_key.flags_changed(KEYCODE, SHIFT), Some(Signal::Release));
    }

    #[test]
    fn fn_with_another_key_is_a_combination() {
        let mut fn_key = Tracker::default();
        // Typing without fn means nothing.
        assert_eq!(fn_key.key_down(), None);
        fn_key.flags_changed(KEYCODE, SECONDARY_FN);
        assert_eq!(fn_key.key_down(), Some(Signal::Combined));
        // Holding fn + ← repeats the arrow: once is enough.
        assert_eq!(fn_key.key_down(), None);
        assert_eq!(fn_key.flags_changed(KEYCODE, 0), Some(Signal::Release));
        // The next press starts fresh.
        fn_key.flags_changed(KEYCODE, SECONDARY_FN);
        assert_eq!(fn_key.key_down(), Some(Signal::Combined));
    }

    #[test]
    fn a_paused_tap_lets_go_of_fn() {
        let mut fn_key = Tracker::default();
        assert_eq!(fn_key.reset(), None);
        fn_key.flags_changed(KEYCODE, SECONDARY_FN);
        assert_eq!(fn_key.reset(), Some(Signal::Release));
        assert_eq!(fn_key.flags_changed(KEYCODE, 0), None);
    }
}
