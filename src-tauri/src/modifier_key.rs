//! A key used alone as the dictation shortcut, on macOS: right ⌘, right ⌥,
//! right ⌃ or fn (🌐).
//!
//! The global-shortcut plugin can't register a modifier on its own, so an
//! event tap follows the key instead. macOS reports a modifier as a "flags
//! changed" event carrying its keycode; whether it is down comes from its own
//! device bit in the event's flags, so left and right of the same family
//! never mix up.
//!
//! A modifier is also half of every chord (fn + ←, right ⌘ + C). The key
//! therefore counts as a press only once it has been held alone for `ARM`,
//! or tapped cleanly: any other key or click in between, and nothing starts.
//! A key pressed a little later, within `CHORD`, is a slower chord: it sends
//! `Interrupted`, which drops what the press began. After that, and for
//! clicks as soon as the press has started, nothing cuts the dictation: you
//! can click, switch windows or scroll while you speak, as in VoiceInk.
//! Presses and releases go to the dictation loop like the plugin's, so every
//! shortcut mode behaves the same.
//!
//! The tap needs the Accessibility permission Parler already asks for. Until
//! it has it, the window says so and the tap is retried every few seconds,
//! so the key works as soon as the permission is granted.

use crate::dictation::{Dictation, Event};
use crate::shortcut::Key;
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// How long the key must be held alone before it counts as a press. Chords
/// usually land within it.
pub const ARM: Duration = Duration::from_millis(150);
/// Until this long after the key went down, another key makes it a chord.
/// Later on, the hold is a dictation and other keys leave it alone.
pub const CHORD: Duration = Duration::from_millis(500);
/// How often a tap that couldn't be created is tried again.
const RETRY: Duration = Duration::from_secs(2);
/// The longest the tap's thread sleeps before checking it should stop.
const SLICE: Duration = Duration::from_millis(500);
/// How often the thread checks the Accessibility permission is still there.
const TRUST_CHECK: Duration = Duration::from_secs(2);

/// The device-dependent bits of every modifier (`NX_DEVICE*KEYMASK`).
const DEVICE_BITS: u64 = 0x207F;

impl Key {
    /// The `kVK_*` code macOS puts on the key's flags-changed events.
    fn keycode(self) -> i64 {
        match self {
            Key::Fn => 63,
            Key::RightCommand => 54,
            Key::RightOption => 61,
            Key::RightControl => 62,
        }
    }

    /// Set while this very key is down (`NX_DEVICER*KEYMASK`, or
    /// `kCGEventFlagMaskSecondaryFn`, which fn has alone).
    fn device_bit(self) -> u64 {
        match self {
            Key::Fn => 0x0080_0000,
            Key::RightCommand => 0x10,
            Key::RightOption => 0x40,
            Key::RightControl => 0x2000,
        }
    }

    /// Set while either key of the family is down.
    fn family_bit(self) -> u64 {
        match self {
            Key::Fn => 0x0080_0000,
            Key::RightCommand => 0x0010_0000,
            Key::RightOption => 0x0008_0000,
            Key::RightControl => 0x0004_0000,
        }
    }

    fn is_down(self, flags: u64) -> bool {
        // Some synthetic or remote events carry only the family bits.
        let bit = if self == Key::Fn || flags & DEVICE_BITS != 0 {
            self.device_bit()
        } else {
            self.family_bit()
        };
        flags & bit != 0
    }
}

/// What the key's events mean for the dictation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Press,
    Release,
    /// Another key came soon after the press: the key was a modifier, not a
    /// request to dictate.
    Interrupted,
}

/// Something else that happened while the key was down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key,
    Click,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Up,
    /// Down, not yet long enough to tell a press from a chord.
    Armed {
        since: Instant,
    },
    /// A press, since the key went down.
    Pressed {
        since: Instant,
    },
    /// Part of a chord: nothing more until the key is up.
    Combined,
}

/// Follows one key through the events of a tap. Time is passed in, so the
/// arm window can be tested without waiting.
#[derive(Debug)]
pub struct Tracker {
    key: Key,
    state: State,
}

impl Tracker {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            state: State::Up,
        }
    }

    /// A modifier changed. Only the key's own events count: arrows and the
    /// F keys carry the fn flag too, and the left keys share the families.
    pub fn flags_changed(&mut self, keycode: i64, flags: u64, now: Instant) -> &'static [Signal] {
        if keycode != self.key.keycode() {
            return &[];
        }
        let down = self.key.is_down(flags);
        match (self.state, down) {
            (State::Up, true) => {
                self.state = State::Armed { since: now };
                &[]
            }
            (State::Armed { .. }, false) => {
                // A clean tap.
                self.state = State::Up;
                &[Signal::Press, Signal::Release]
            }
            (State::Pressed { .. }, false) => {
                self.state = State::Up;
                &[Signal::Release]
            }
            (State::Combined, false) => {
                self.state = State::Up;
                &[]
            }
            // Already down, or up before we ever saw it go down.
            _ => &[],
        }
    }

    /// Another key went down, or a mouse button.
    pub fn other_input(&mut self, input: Input, now: Instant) -> Option<Signal> {
        match self.state {
            State::Armed { .. } => {
                self.state = State::Combined;
                None
            }
            State::Pressed { since }
                if input == Input::Key && now.duration_since(since) < CHORD =>
            {
                self.state = State::Combined;
                Some(Signal::Interrupted)
            }
            // A click, or a key once the dictation is under way.
            State::Pressed { .. } | State::Up | State::Combined => None,
        }
    }

    /// Time passed: held alone long enough, the key is a press.
    pub fn tick(&mut self, now: Instant) -> Option<Signal> {
        match self.state {
            State::Armed { since } if now.duration_since(since) >= ARM => {
                self.state = State::Pressed { since };
                Some(Signal::Press)
            }
            _ => None,
        }
    }

    /// When `tick` should next be called.
    pub fn deadline(&self) -> Option<Instant> {
        match self.state {
            State::Armed { since } => Some(since + ARM),
            _ => None,
        }
    }

    /// The tap was paused, so a release may have been missed: assume the
    /// key is up rather than leave a dictation running.
    pub fn reset(&mut self) -> Option<Signal> {
        let was = std::mem::replace(&mut self.state, State::Up);
        matches!(was, State::Pressed { .. }).then_some(Signal::Release)
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFPreferencesCopyAppValue(key: CFStringRef, application: CFStringRef) -> CFTypeRef;
    fn CFPreferencesAppSynchronize(application: CFStringRef) -> bool;
}

/// What macOS does with a 🌐 press (System Settings → Keyboard): 0 nothing,
/// 1 input source, 2 emoji, 3 Dictation; `None` when never changed, which
/// means the system default (emoji).
pub fn globe_action() -> Option<i64> {
    let domain = CFString::from_static_string("com.apple.HIToolbox");
    let key = CFString::from_static_string("AppleFnUsageType");
    unsafe {
        // Picks up a change made in System Settings since the last read.
        CFPreferencesAppSynchronize(domain.as_concrete_TypeRef());
        let value =
            CFPreferencesCopyAppValue(key.as_concrete_TypeRef(), domain.as_concrete_TypeRef());
        if value.is_null() {
            return None;
        }
        let action = if core_foundation::base::CFGetTypeID(value) == CFNumber::type_id() {
            CFNumber::wrap_under_get_rule(value as CFNumberRef).to_i64()
        } else {
            None
        };
        CFRelease(value);
        action
    }
}

struct Running {
    key: Key,
    stop: Arc<AtomicBool>,
    run_loop: Arc<Mutex<Option<CFRunLoop>>>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Starts following `key`, instead of any other key.
pub fn start(app: &AppHandle, key: Key) {
    let mut running = lock(&RUNNING);
    match running.as_ref() {
        Some(current) if current.key == key => return,
        Some(_) => stop_locked(&mut running),
        None => {}
    }
    let stop = Arc::new(AtomicBool::new(false));
    let run_loop = Arc::new(Mutex::new(None));
    let spawned = thread::Builder::new().name("modifier-key".into()).spawn({
        let app = app.clone();
        let stop = stop.clone();
        let run_loop = run_loop.clone();
        move || listen(app, key, stop, run_loop)
    });
    match spawned {
        Ok(_) => {
            *running = Some(Running {
                key,
                stop,
                run_loop,
            })
        }
        Err(e) => log::warn!("Could not start listening to {key:?}: {e}"),
    }
}

/// Stops following the key; another shortcut takes over.
pub fn stop() {
    stop_locked(&mut lock(&RUNNING));
}

fn stop_locked(running: &mut Option<Running>) {
    if let Some(running) = running.take() {
        running.stop.store(true, Ordering::SeqCst);
        if let Some(run_loop) = lock(&running.run_loop).as_ref() {
            run_loop.stop();
        }
    }
}

pub fn is_running(key: Key) -> bool {
    lock(&RUNNING).as_ref().is_some_and(|r| r.key == key)
}

fn listen(
    app: AppHandle,
    key: Key,
    stop: Arc<AtomicBool>,
    run_loop: Arc<Mutex<Option<CFRunLoop>>>,
) {
    let tracker = Arc::new(Mutex::new(Tracker::new(key)));
    let mut refused = false;
    while !stop.load(Ordering::SeqCst) {
        let Ok(tap) = create_tap(&app, key, &tracker, &stop) else {
            if !refused {
                refused = true;
                log::warn!("{key:?} needs Accessibility access; retrying until granted");
                crate::shortcut::report_problem(
                    &app,
                    key.setting(),
                    "Accessibility access is needed to listen to the key".to_string(),
                );
            }
            thread::sleep(RETRY);
            continue;
        };
        if std::mem::take(&mut refused) {
            crate::shortcut::clear_problem(&app);
        }
        let Ok(source) = tap.mach_port().create_runloop_source(0) else {
            log::warn!("Could not listen to {key:?}: no run loop source");
            return;
        };
        let current = CFRunLoop::get_current();
        current.add_source(&source, unsafe { kCFRunLoopCommonModes });
        tap.enable();
        *lock(&run_loop) = Some(current.clone());
        log::info!("Listening to {key:?}");

        let mut trust_checked = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            // Wake up for the arm deadline, or at least every slice.
            let wait = lock(&tracker)
                .deadline()
                .map_or(SLICE, |deadline| {
                    deadline.saturating_duration_since(Instant::now())
                })
                .min(SLICE);
            CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, wait, true);
            let signal = lock(&tracker).tick(Instant::now());
            send(&app, signal);

            // A permission taken back leaves a tap that hears nothing: say so
            // and wait for it again.
            if trust_checked.elapsed() >= TRUST_CHECK {
                trust_checked = Instant::now();
                if !unsafe { AXIsProcessTrusted() } {
                    log::warn!("Accessibility access was removed; {key:?} no longer works");
                    send(&app, lock(&tracker).reset());
                    refused = true;
                    crate::shortcut::report_problem(
                        &app,
                        key.setting(),
                        "Accessibility access is needed to listen to the key".to_string(),
                    );
                    break;
                }
            }
        }
        current.remove_source(&source, unsafe { kCFRunLoopCommonModes });
        // Dropping the tap removes it.
    }
    log::info!("Stopped listening to {key:?}");
}

fn send(app: &AppHandle, signal: Option<Signal>) {
    if let Some(signal) = signal {
        app.state::<Dictation>().send(match signal {
            Signal::Press => Event::Press,
            Signal::Release => Event::Release,
            Signal::Interrupted => Event::Interrupted,
        });
    }
}

fn create_tap(
    app: &AppHandle,
    key: Key,
    tracker: &Arc<Mutex<Tracker>>,
    stop: &Arc<AtomicBool>,
) -> Result<CGEventTap<'static>, ()> {
    // Filled in once the tap exists, so its callback can turn it back on.
    let port = Arc::new(AtomicPtr::<c_void>::new(std::ptr::null_mut()));
    let app = app.clone();
    let stop = stop.clone();
    let tracker = tracker.clone();
    let callback_port = port.clone();
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        // An active tap is what Accessibility allows; events pass unchanged.
        CGEventTapOptions::Default,
        vec![
            CGEventType::FlagsChanged,
            CGEventType::KeyDown,
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::OtherMouseDown,
        ],
        move |_proxy, kind, event| {
            if stop.load(Ordering::Relaxed) {
                return CallbackResult::Keep;
            }
            let mut tracker = lock(&tracker);
            match kind {
                CGEventType::FlagsChanged => {
                    let signals = tracker.flags_changed(
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
                        event.get_flags().bits(),
                        Instant::now(),
                    );
                    for signal in signals {
                        send(&app, Some(*signal));
                    }
                }
                CGEventType::KeyDown => send(&app, tracker.other_input(Input::Key, Instant::now())),
                CGEventType::LeftMouseDown
                | CGEventType::RightMouseDown
                | CGEventType::OtherMouseDown => {
                    send(&app, tracker.other_input(Input::Click, Instant::now()))
                }
                // macOS pauses a tap it finds slow, or during secure input.
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    let tap = callback_port.load(Ordering::SeqCst);
                    if !tap.is_null() {
                        log::info!("The {key:?} listener was paused; resuming");
                        unsafe { CGEventTapEnable(tap as CFMachPortRef, true) };
                    }
                    send(&app, tracker.reset());
                }
                _ => {}
            }
            CallbackResult::Keep
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

    const FN_DOWN: u64 = 0x0080_0000;
    /// Right ⌘ alone: the ⌘ family bit and its device bit.
    const RIGHT_CMD_DOWN: u64 = 0x0010_0010;
    /// Left ⌘ alone.
    const LEFT_CMD_DOWN: u64 = 0x0010_0008;

    fn ms(start: Instant, n: u64) -> Instant {
        start + Duration::from_millis(n)
    }

    #[test]
    fn a_clean_tap_is_a_press_and_a_release() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        assert_eq!(key.flags_changed(63, FN_DOWN, t0), &[]);
        assert_eq!(key.tick(ms(t0, 80)), None);
        assert_eq!(
            key.flags_changed(63, 0, ms(t0, 100)),
            &[Signal::Press, Signal::Release]
        );
    }

    #[test]
    fn a_hold_is_a_press_once_armed() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::RightCommand);
        key.flags_changed(54, RIGHT_CMD_DOWN, t0);
        assert_eq!(key.deadline(), Some(ms(t0, 150)));
        assert_eq!(key.tick(ms(t0, 149)), None);
        assert_eq!(key.tick(ms(t0, 150)), Some(Signal::Press));
        assert_eq!(key.deadline(), None);
        assert_eq!(key.tick(ms(t0, 900)), None);
        assert_eq!(key.flags_changed(54, 0, ms(t0, 1200)), &[Signal::Release]);
    }

    #[test]
    fn a_quick_chord_starts_nothing() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        key.flags_changed(63, FN_DOWN, t0);
        // fn + ← within the arm window.
        assert_eq!(key.other_input(Input::Key, ms(t0, 60)), None);
        assert_eq!(key.tick(ms(t0, 400)), None);
        assert_eq!(key.flags_changed(63, 0, ms(t0, 500)), &[]);
    }

    #[test]
    fn a_slow_chord_is_interrupted_once() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        key.flags_changed(63, FN_DOWN, t0);
        assert_eq!(key.tick(ms(t0, 160)), Some(Signal::Press));
        assert_eq!(
            key.other_input(Input::Key, ms(t0, 300)),
            Some(Signal::Interrupted)
        );
        // Holding fn + ← repeats the arrow: once is enough.
        assert_eq!(key.other_input(Input::Key, ms(t0, 350)), None);
        assert_eq!(key.flags_changed(63, 0, ms(t0, 700)), &[]);
    }

    #[test]
    fn a_click_in_the_arm_window_is_a_chord() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::RightCommand);
        key.flags_changed(54, RIGHT_CMD_DOWN, t0);
        // ⌘-click to open a link in a new tab.
        assert_eq!(key.other_input(Input::Click, ms(t0, 90)), None);
        assert_eq!(key.tick(ms(t0, 400)), None);
        assert_eq!(key.flags_changed(54, 0, ms(t0, 500)), &[]);
    }

    #[test]
    fn clicks_never_cut_a_dictation() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        key.flags_changed(63, FN_DOWN, t0);
        assert_eq!(key.tick(ms(t0, 150)), Some(Signal::Press));
        // Switching windows while speaking.
        assert_eq!(key.other_input(Input::Click, ms(t0, 200)), None);
        assert_eq!(key.other_input(Input::Click, ms(t0, 2000)), None);
        assert_eq!(key.flags_changed(63, 0, ms(t0, 4000)), &[Signal::Release]);
    }

    #[test]
    fn keys_leave_a_dictation_under_way_alone() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        key.flags_changed(63, FN_DOWN, t0);
        key.tick(ms(t0, 150));
        // ⌘-Tab, well after the press.
        assert_eq!(key.other_input(Input::Key, ms(t0, 500)), None);
        assert_eq!(key.other_input(Input::Key, ms(t0, 1500)), None);
        assert_eq!(key.flags_changed(63, 0, ms(t0, 3000)), &[Signal::Release]);
    }

    #[test]
    fn the_left_key_of_the_family_is_ignored() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::RightCommand);
        assert_eq!(key.flags_changed(55, LEFT_CMD_DOWN, t0), &[]);
        assert_eq!(key.deadline(), None);
        // Keycode 54 with only the left device bit: not our key going down.
        assert_eq!(key.flags_changed(54, LEFT_CMD_DOWN, t0), &[]);
        assert_eq!(key.deadline(), None);
    }

    #[test]
    fn right_released_while_left_is_held() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::RightCommand);
        key.flags_changed(54, RIGHT_CMD_DOWN | LEFT_CMD_DOWN, t0);
        key.tick(ms(t0, 200));
        // Left ⌘ still down: the family bit stays, the right device bit goes.
        assert_eq!(
            key.flags_changed(54, LEFT_CMD_DOWN, ms(t0, 600)),
            &[Signal::Release]
        );
    }

    #[test]
    fn events_with_only_family_bits_still_work() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::RightOption);
        key.flags_changed(61, 0x0008_0000, t0);
        assert_eq!(
            key.flags_changed(61, 0, ms(t0, 50)),
            &[Signal::Press, Signal::Release]
        );
    }

    #[test]
    fn other_keys_with_the_fn_flag_are_ignored() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        // Shift (56) changing while an arrow sets the fn flag.
        assert_eq!(key.flags_changed(56, 0x0002_0002 | FN_DOWN, t0), &[]);
        assert_eq!(key.deadline(), None);
    }

    #[test]
    fn a_first_event_up_does_nothing() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::RightControl);
        assert_eq!(key.flags_changed(62, 0, t0), &[]);
        assert_eq!(key.other_input(Input::Key, t0), None);
    }

    #[test]
    fn a_paused_tap_releases_only_a_press() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        assert_eq!(key.reset(), None);
        key.flags_changed(63, FN_DOWN, t0);
        assert_eq!(key.reset(), None);
        key.flags_changed(63, FN_DOWN, t0);
        key.tick(ms(t0, 150));
        assert_eq!(key.reset(), Some(Signal::Release));
        assert_eq!(key.flags_changed(63, 0, ms(t0, 300)), &[]);
    }

    #[test]
    fn repeated_down_events_count_once() {
        let t0 = Instant::now();
        let mut key = Tracker::new(Key::Fn);
        key.flags_changed(63, FN_DOWN, t0);
        assert_eq!(key.flags_changed(63, FN_DOWN, ms(t0, 100)), &[]);
        assert_eq!(key.deadline(), Some(ms(t0, 150)));
    }
}
