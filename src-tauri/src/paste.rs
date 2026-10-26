//! Puts the transcript where the user is typing: through the clipboard and a
//! simulated Cmd+V / Ctrl+V, then puts back what the clipboard held before.
//!
//! The clipboard is saved as fully as the platform allows: every item with
//! every type on macOS; a file list, text with its HTML, or an image on
//! Windows and Linux. The transcript is marked so clipboard histories skip
//! it, and the clipboard is put back only if it still holds the transcript.
//!
//! The layout-aware Cmd+V lookup is adapted from Handy
//! (https://github.com/cjpais/Handy, MIT, Copyright (c) 2025 CJ Pais).

use enigo::{Direction, Enigo, Key, Keyboard};
use serde::Serialize;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use pasteboard::{Ours, Saved};
#[cfg(not(target_os = "macos"))]
use portable::{Ours, Saved};

/// One clipboard for the app's lifetime: on Linux the clipboard's content
/// lives only as long as the process owning it keeps serving it.
pub struct ClipboardState {
    clipboard: Mutex<Option<arboard::Clipboard>>,
    /// Held for a whole paste: a second one starting meanwhile would save
    /// the first transcript as the user's clipboard.
    pasting: Mutex<()>,
}

impl ClipboardState {
    pub fn new() -> Self {
        let clipboard = arboard::Clipboard::new()
            .inspect_err(|e| log::warn!("Clipboard unavailable: {e}"))
            .ok();
        Self {
            clipboard: Mutex::new(clipboard),
            pasting: Mutex::new(()),
        }
    }

    fn with<T>(
        &self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, PasteError> {
        let mut guard = self.clipboard.lock().unwrap_or_else(|e| e.into_inner());
        let clipboard = guard.as_mut().ok_or(PasteError::NoClipboard)?;
        f(clipboard).map_err(|e| PasteError::Failed {
            detail: e.to_string(),
        })
    }

    /// Once the target app has pasted.
    fn restore(&self, previous: Previous<Saved>, ours: &Ours) {
        let restored = match plan_restore(previous, self.holds(ours)) {
            Restore::Put(saved) => self.put_back(saved),
            Restore::Clear => self.clear(),
            Restore::Leave => Ok(()),
        };
        if let Err(e) = restored {
            log::warn!("Could not restore the clipboard: {e:?}");
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PasteError {
    NoClipboard,
    /// macOS: Parler may not send keystrokes until the user allows it in
    /// Privacy & Security > Accessibility. The transcript is left on the
    /// clipboard.
    NeedsAccessibility,
    Failed {
        detail: String,
    },
}

/// A plain copy, which clipboard histories keep: copy mode and "Copy last
/// transcript" are meant to leave the text there.
pub fn copy(app: &AppHandle, text: &str) -> Result<(), PasteError> {
    let text = text.to_string();
    app.state::<ClipboardState>()
        .with(move |clipboard| clipboard.set_text(text))
}

/// Blocks for a few hundred milliseconds; never call it on the main thread,
/// which it needs free to send the keystrokes.
pub fn paste(app: &AppHandle, text: &str) -> Result<(), PasteError> {
    let clipboard = app.state::<ClipboardState>();
    let _pasting = clipboard.pasting.lock().unwrap_or_else(|e| e.into_inner());
    let previous = clipboard.save();
    let ours = clipboard.write_transient(text)?;
    // Give the clipboard a moment to settle before the target reads it.
    thread::sleep(Duration::from_millis(60));

    // On failure the transcript stays on the clipboard so the user can paste
    // it by hand; that beats restoring and losing it.
    send_paste_keys(app)?;

    // Let the target app read the clipboard before restoring it.
    thread::sleep(Duration::from_millis(350));
    clipboard.restore(previous, &ours);
    Ok(())
}

/// What the clipboard held before the transcript replaced it.
#[derive(Debug, PartialEq)]
enum Previous<T> {
    Empty,
    Saved(T),
    /// Content in a format Parler can't read, or a clipboard it couldn't
    /// open.
    Unreadable,
}

#[derive(Debug, PartialEq)]
enum Restore<T> {
    Put(T),
    Clear,
    Leave,
}

/// Touch the clipboard only while it still holds the transcript: anything
/// else was copied meanwhile and is the user's. Then put back what was
/// saved, or clear a clipboard that was empty. Content Parler couldn't read
/// is gone either way, and the transcript beats an empty clipboard.
fn plan_restore<T>(previous: Previous<T>, still_ours: bool) -> Restore<T> {
    match previous {
        _ if !still_ours => Restore::Leave,
        Previous::Empty => Restore::Clear,
        Previous::Saved(saved) => Restore::Put(saved),
        Previous::Unreadable => Restore::Leave,
    }
}

fn send_paste_keys(app: &AppHandle) -> Result<(), PasteError> {
    #[cfg(target_os = "linux")]
    if linux::send_with_native_tool() {
        return Ok(());
    }

    // Enigo and the macOS keyboard-layout APIs want the main thread.
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(press_paste_chord());
    })
    .map_err(|e| PasteError::Failed {
        detail: e.to_string(),
    })?;
    rx.recv().map_err(|_| PasteError::Failed {
        detail: "the main thread dropped the paste".to_string(),
    })?
}

fn press_paste_chord() -> Result<(), PasteError> {
    let mut enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| {
        if cfg!(target_os = "macos") {
            log::warn!("Cannot send keystrokes: {e}");
            PasteError::NeedsAccessibility
        } else {
            PasteError::Failed {
                detail: e.to_string(),
            }
        }
    })?;

    #[cfg(target_os = "macos")]
    let (modifier, v) = (Key::Meta, macos::command_v_key());
    #[cfg(target_os = "windows")]
    let (modifier, v) = (Key::Control, Key::Other(0x56)); // VK_V
    #[cfg(target_os = "linux")]
    let (modifier, v) = (Key::Control, Key::Unicode('v'));

    let failed = |e: enigo::InputError| PasteError::Failed {
        detail: e.to_string(),
    };
    enigo.key(modifier, Direction::Press).map_err(failed)?;
    let clicked = enigo.key(v, Direction::Click).map_err(failed);
    // Some apps read the modifier from global state when handling the V,
    // so it stays down a moment; it is released even if the click failed.
    thread::sleep(Duration::from_millis(60));
    enigo.key(modifier, Direction::Release).map_err(failed)?;
    clicked
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::{Command, Stdio};

    fn run(program: &str, args: &[&str]) -> bool {
        Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Wayland ignores simulated X11 input for native windows; use a
    /// compositor-level tool when one is installed.
    pub fn send_with_native_tool() -> bool {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return false;
        }
        run("wtype", &["-M", "ctrl", "v", "-m", "ctrl"])
            || run("ydotool", &["key", "29:1", "47:1", "47:0", "29:0"])
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::Key;
    use std::ffi::c_void;

    type TisInputSourceRef = *const c_void;
    type CfDataRef = *const c_void;
    type CfStringRef = *const c_void;

    /// kVK_ANSI_V, the fallback when the active layout can't be read.
    const ANSI_V_KEYCODE: u16 = 9;
    const KEYCODE_COUNT: u16 = 128;
    const UC_KEY_ACTION_DISPLAY: u16 = 3;
    const UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;
    // Carbon's cmdKey (bit 8), shifted right by 8 as UCKeyTranslate expects.
    const COMMAND_MODIFIER_STATE: u32 = 1;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardLayoutInputSource() -> TisInputSourceRef;
        fn TISGetInputSourceProperty(
            input_source: TisInputSourceRef,
            property_key: CfStringRef,
        ) -> CfDataRef;
        static kTISPropertyUnicodeKeyLayoutData: CfStringRef;
        fn UCKeyTranslate(
            key_layout: *const u8,
            virtual_key_code: u16,
            key_action: u16,
            modifier_key_state: u32,
            keyboard_type: u32,
            key_translate_options: u32,
            dead_key_state: *mut u32,
            max_string_length: usize,
            actual_string_length: *mut usize,
            unicode_string: *mut u16,
        ) -> i32;
        fn LMGetKbdType() -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataGetBytePtr(data: CfDataRef) -> *const u8;
        fn CFRelease(value: *const c_void);
    }

    struct InputSource(TisInputSourceRef);

    impl Drop for InputSource {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: balances the retain from
                // TISCopyCurrentKeyboardLayoutInputSource.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    /// The physical key macOS reads as `v` while Command is held. Asking with
    /// Command matters: non-Latin layouts map Cmd shortcuts to their ANSI
    /// keys, while Dvorak does not. Must run on the main thread.
    fn command_v_keycode() -> Option<u16> {
        // SAFETY: called on the main thread; released by InputSource::drop.
        let source = InputSource(unsafe { TISCopyCurrentKeyboardLayoutInputSource() });
        if source.0.is_null() {
            return None;
        }
        // SAFETY: the source stays retained for the whole scan.
        let data = unsafe { TISGetInputSourceProperty(source.0, kTISPropertyUnicodeKeyLayoutData) };
        if data.is_null() {
            return None;
        }
        // SAFETY: data is owned by the retained source.
        let layout = unsafe { CFDataGetBytePtr(data) };
        if layout.is_null() {
            return None;
        }
        // SAFETY: no arguments; returns the current keyboard type.
        let keyboard_type = unsafe { LMGetKbdType() } as u32;

        (0..KEYCODE_COUNT).find(|&keycode| {
            let mut dead_key_state = 0;
            let mut chars = [0_u16; 4];
            let mut length = 0_usize;
            // SAFETY: layout is valid while source is retained; every output
            // pointer is local storage of the declared size.
            let status = unsafe {
                UCKeyTranslate(
                    layout,
                    keycode,
                    UC_KEY_ACTION_DISPLAY,
                    COMMAND_MODIFIER_STATE,
                    keyboard_type,
                    UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                    &mut dead_key_state,
                    chars.len(),
                    &mut length,
                    chars.as_mut_ptr(),
                )
            };
            status == 0 && length == 1 && chars[0] == u16::from(b'v')
        })
    }

    pub(super) fn command_v_key() -> Key {
        let keycode = command_v_keycode().unwrap_or_else(|| {
            log::warn!("Could not read the keyboard layout; using the ANSI V key");
            ANSI_V_KEYCODE
        });
        Key::Other(u32::from(keycode))
    }
}

/// Windows and Linux save what arboard reads, and write back the same kind.
/// Other formats (RTF, an app's own) are lost, and a cut in a file manager
/// comes back as a copy.
#[cfg(not(target_os = "macos"))]
mod portable {
    use super::{ClipboardState, PasteError, Previous};
    use std::path::PathBuf;

    pub(super) enum Saved {
        Files(Vec<PathBuf>),
        Text { text: String, html: Option<String> },
        Image(arboard::ImageData<'static>),
    }

    /// The transcript itself, which the clipboard must still hold.
    pub(super) type Ours = String;

    impl ClipboardState {
        pub(super) fn save(&self) -> Previous<Saved> {
            self.with(|clipboard| Ok(read(clipboard)))
                .unwrap_or(Previous::Unreadable)
        }

        /// Kept out of clipboard histories (and Windows' cloud clipboard):
        /// the transcript only passes through.
        pub(super) fn write_transient(&self, text: &str) -> Result<Ours, PasteError> {
            self.with(|clipboard| {
                let set = clipboard.set();
                #[cfg(target_os = "linux")]
                let set = arboard::SetExtLinux::exclude_from_history(set);
                // Monitoring keeps third-party clipboard managers away;
                // history and cloud are Windows' own switches. Password
                // managers set all three, as some tools check only one.
                #[cfg(windows)]
                let set = {
                    use arboard::SetExtWindows as _;
                    set.exclude_from_monitoring()
                        .exclude_from_cloud()
                        .exclude_from_history()
                };
                set.text(text)
            })?;
            Ok(text.to_string())
        }

        pub(super) fn holds(&self, ours: &Ours) -> bool {
            self.with(|clipboard| clipboard.get_text())
                .is_ok_and(|text| text == *ours)
        }

        pub(super) fn put_back(&self, saved: Saved) -> Result<(), PasteError> {
            self.with(move |clipboard| match saved {
                Saved::Files(files) => clipboard.set().file_list(&files),
                Saved::Text {
                    text,
                    html: Some(html),
                } => clipboard.set_html(html, Some(text)),
                Saved::Text { text, html: None } => clipboard.set_text(text),
                Saved::Image(image) => clipboard.set_image(image),
            })
        }

        pub(super) fn clear(&self) -> Result<(), PasteError> {
            self.with(|clipboard| clipboard.clear())
        }
    }

    /// Files first: file managers add their paths as text. Text before an
    /// image: spreadsheets and word processors add a picture of the
    /// selection.
    fn read(clipboard: &mut arboard::Clipboard) -> Previous<Saved> {
        let files = clipboard.get().file_list().unwrap_or_default();
        if !files.is_empty() {
            return Previous::Saved(Saved::Files(files));
        }
        if let Ok(text) = clipboard.get_text() {
            let html = clipboard.get().html().ok();
            return Previous::Saved(Saved::Text { text, html });
        }
        if let Ok(image) = clipboard.get_image() {
            return Previous::Saved(Saved::Image(image));
        }
        if is_empty() {
            Previous::Empty
        } else {
            Previous::Unreadable
        }
    }

    #[cfg(windows)]
    fn is_empty() -> bool {
        // SAFETY: takes nothing; it only counts the formats on the clipboard.
        unsafe { windows::Win32::System::DataExchange::CountClipboardFormats() == 0 }
    }

    /// X11 says "not available" alike for an empty clipboard and a format
    /// we can't read, so both leave the transcript.
    #[cfg(not(windows))]
    fn is_empty() -> bool {
        false
    }
}

/// macOS saves the general pasteboard as raw data, every item with every
/// type but file promises, so a screenshot, files copied in Finder or rich
/// text come back exactly as they were.
///
/// These run on the paste thread, which has no autorelease pool of its own:
/// each call drains one, or what AppKit autoreleases would leak.
#[cfg(target_os = "macos")]
mod pasteboard {
    use super::{ClipboardState, PasteError, Previous};
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardContentsOptions, NSPasteboardItem, NSPasteboardTypeString,
        NSPasteboardWriting,
    };
    use objc2_foundation::{ns_string, NSArray, NSData, NSString};

    /// Each item's types, in order, with their data.
    pub(super) type Saved = Vec<Vec<(Retained<NSString>, Retained<NSData>)>>;

    /// The change count right after our write: any later copy moves it, even
    /// one of the same text.
    pub(super) type Ours = isize;

    /// Reading a file promise makes the app behind it (Photos, Mail, Finder)
    /// write the file out, on every dictation. So promises are never read:
    /// such an item comes back without them, and can no longer be pasted as
    /// a file, though the rest of it (an image, a name as text) does.
    pub(super) fn is_file_promise(kind: &str) -> bool {
        kind.starts_with("com.apple.pasteboard.promised-")
            || matches!(
                kind,
                "com.apple.NSFilePromiseItemMetaData"
                    | "Apple files promise pasteboard type"
                    | "NSPromiseContentsPboardType"
                    // The two legacy names as the pasteboard reports them.
                    | "dyn.ah62d4rv4gu8yc6durvwwa3xmrvw1gkdusm1044pxqyuha2pxsvw0e55bsmwca7d3sbwu"
                    | "dyn.ah62d4rv4gu8y6y4usm1044pxqzb085xyqz1hk64uqm10c6xenv61a3k"
            )
    }

    impl ClipboardState {
        pub(super) fn save(&self) -> Previous<Saved> {
            autoreleasepool(|_| {
                let Some(items) = NSPasteboard::generalPasteboard().pasteboardItems() else {
                    return Previous::Unreadable;
                };
                if items.is_empty() {
                    return Previous::Empty;
                }
                // A file promise, or a type whose data can't be had (its app
                // quit before providing it), is skipped; the rest still
                // comes back.
                let saved: Saved = items
                    .iter()
                    .map(|item| {
                        item.types()
                            .iter()
                            .filter(|kind| !is_file_promise(&kind.to_string()))
                            .filter_map(|kind| item.dataForType(&kind).map(|data| (kind, data)))
                            .collect::<Vec<_>>()
                    })
                    .filter(|types| !types.is_empty())
                    .collect();
                if saved.is_empty() {
                    Previous::Unreadable
                } else {
                    Previous::Saved(saved)
                }
            })
        }

        pub(super) fn write_transient(&self, text: &str) -> Result<Ours, PasteError> {
            autoreleasepool(|_| {
                let item = NSPasteboardItem::new();
                // SAFETY: an AppKit constant, set before any code runs.
                let string_type = unsafe { NSPasteboardTypeString };
                let mut ready = item.setString_forType(&NSString::from_str(text), string_type);
                // nspasteboard.org markers, which Maccy, Raycast, Alfred and
                // others honour: the transcript is there only for a moment,
                // and the user never copied it. ConcealedType is for secrets.
                for marker in [
                    ns_string!("org.nspasteboard.TransientType"),
                    ns_string!("org.nspasteboard.AutoGeneratedType"),
                ] {
                    ready &= item.setData_forType(&NSData::new(), marker);
                }
                if !ready {
                    return Err(refused());
                }
                let pasteboard = NSPasteboard::generalPasteboard();
                // Not offered to the user's other devices by Universal
                // Clipboard either.
                pasteboard
                    .prepareForNewContentsWithOptions(NSPasteboardContentsOptions::CurrentHostOnly);
                if write(&pasteboard, vec![item]) {
                    Ok(pasteboard.changeCount())
                } else {
                    Err(refused())
                }
            })
        }

        pub(super) fn holds(&self, ours: &Ours) -> bool {
            autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount() == *ours)
        }

        pub(super) fn put_back(&self, saved: Saved) -> Result<(), PasteError> {
            autoreleasepool(|_| {
                let items = saved
                    .into_iter()
                    .map(|types| {
                        let item = NSPasteboardItem::new();
                        for (kind, data) in types {
                            item.setData_forType(&data, &kind);
                        }
                        item
                    })
                    .collect();
                let pasteboard = NSPasteboard::generalPasteboard();
                pasteboard.clearContents();
                if write(&pasteboard, items) {
                    Ok(())
                } else {
                    Err(refused())
                }
            })
        }

        pub(super) fn clear(&self) -> Result<(), PasteError> {
            autoreleasepool(|_| NSPasteboard::generalPasteboard().clearContents());
            Ok(())
        }
    }

    fn write(pasteboard: &NSPasteboard, items: Vec<Retained<NSPasteboardItem>>) -> bool {
        let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = items
            .into_iter()
            .map(ProtocolObject::from_retained)
            .collect();
        pasteboard.writeObjects(&NSArray::from_retained_slice(&items))
    }

    fn refused() -> PasteError {
        PasteError::Failed {
            detail: "the pasteboard refused the write".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_only_over_the_transcript_and_never_clears_the_unknown() {
        assert_eq!(
            plan_restore(Previous::Saved("photo"), true),
            Restore::Put("photo")
        );
        assert_eq!(plan_restore(Previous::<&str>::Empty, true), Restore::Clear);
        assert_eq!(
            plan_restore(Previous::<&str>::Unreadable, true),
            Restore::Leave
        );
        // Something copied meanwhile is the user's, whatever came before.
        for previous in [
            Previous::Saved("photo"),
            Previous::Empty,
            Previous::Unreadable,
        ] {
            assert_eq!(plan_restore(previous, false), Restore::Leave);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn file_promises_are_never_read() {
        for promise in [
            "com.apple.pasteboard.promised-file-url",
            "com.apple.pasteboard.promised-file-content-type",
            "com.apple.pasteboard.promised-suggested-file-name",
            "com.apple.NSFilePromiseItemMetaData",
            "Apple files promise pasteboard type",
            "dyn.ah62d4rv4gu8yc6durvwwa3xmrvw1gkdusm1044pxqyuha2pxsvw0e55bsmwca7d3sbwu",
        ] {
            assert!(pasteboard::is_file_promise(promise), "{promise}");
        }
        for kept in [
            "public.file-url",
            "public.jpeg",
            "public.utf8-plain-text",
            "com.apple.pasteboard.promised",
            // NSFilenamesPboardType: real paths, not a promise.
            "dyn.ah62d4rv4gu8y6y4grf0gn5xbrzw1gydcr7u1e3cytf2gn",
        ] {
            assert!(!pasteboard::is_file_promise(kept), "{kept}");
        }
    }

    /// Run by hand with something on the clipboard (a screenshot, files
    /// copied in Finder, rich text, or nothing), which it puts back:
    /// `cargo test --no-default-features -- --ignored`
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "uses the real pasteboard"]
    fn the_pasteboard_comes_back_as_it_was() {
        type Raw = Vec<Vec<(String, Vec<u8>)>>;
        let clipboard = ClipboardState::new();
        let raw = || -> Raw {
            match clipboard.save() {
                Previous::Saved(items) => items
                    .iter()
                    .map(|types| {
                        types
                            .iter()
                            .map(|(kind, data)| (kind.to_string(), data.to_vec()))
                            .collect()
                    })
                    .collect(),
                _ => Vec::new(),
            }
        };
        let before = raw();
        // Every type and byte saved is back, item by item. macOS may add a
        // type it derives from them, like UTF-16 text from RTF.
        let all_back = |after: Raw| {
            after.len() == before.len()
                && before
                    .iter()
                    .zip(&after)
                    .all(|(was, now)| was.iter().all(|kind| now.contains(kind)))
        };

        let previous = clipboard.save();
        let ours = clipboard.write_transient("Bonjour Camille").unwrap();
        let written: Vec<String> = raw().concat().into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(
            written,
            [
                "public.utf8-plain-text",
                "org.nspasteboard.TransientType",
                "org.nspasteboard.AutoGeneratedType"
            ]
        );
        clipboard.restore(previous, &ours);
        assert!(all_back(raw()));

        // A copy made while the target app pastes is left alone.
        let original = clipboard.save();
        let previous = clipboard.save();
        let ours = clipboard.write_transient("Bonjour Camille").unwrap();
        clipboard.with(|c| c.set_text("copied meanwhile")).unwrap();
        clipboard.restore(previous, &ours);
        assert_eq!(
            clipboard.with(|c| c.get_text()).unwrap(),
            "copied meanwhile"
        );

        match original {
            Previous::Saved(saved) => clipboard.put_back(saved).unwrap(),
            Previous::Empty => clipboard.clear().unwrap(),
            Previous::Unreadable => {}
        }
        assert!(all_back(raw()));
    }
}
