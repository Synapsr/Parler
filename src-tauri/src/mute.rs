//! Silences the computer while a dictation is recorded: music, videos and
//! calls go quiet, then come back as they were.
//!
//! The default output device is muted rather than the media paused: muting
//! works with every app, while pausing needs each player's cooperation (and
//! a play/pause key would start the music when none was playing). A device
//! the person had already muted is left alone, so their own mute is never
//! undone.
//!
//! A file in the data directory says Parler muted the sound; should Parler
//! quit or crash before putting it back, the next launch does.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Named while the sound is muted by Parler, in the app's data directory.
const MARKER: &str = "muted-output";

static MARKER_PATH: OnceLock<PathBuf> = OnceLock::new();
/// What Parler muted, to unmute it and nothing else.
static MUTED: Mutex<Option<platform::Muted>> = Mutex::new(None);

/// At launch: puts back the sound a previous run muted and never unmuted.
pub fn init(data_dir: PathBuf) {
    let marker = data_dir.join(MARKER);
    if marker.exists() {
        log::info!("Unmuting the sound left muted by the last run");
        platform::unmute_default();
        let _ = fs::remove_file(&marker);
    }
    let _ = MARKER_PATH.set(marker);
}

/// Mutes the default output device, unless it is muted already.
pub fn mute() {
    let mut muted = lock();
    if muted.is_some() {
        return;
    }
    // Written first: a crash right after muting still leaves it behind.
    let marker = MARKER_PATH.get();
    if let Some(marker) = marker {
        let written = marker
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(marker, b""));
        if let Err(e) = written {
            log::warn!("Couldn't note the muted sound: {e}");
        }
    }
    match platform::mute() {
        Ok(Some(device)) => *muted = Some(device),
        Ok(None) => remove_marker(),
        Err(e) => {
            log::warn!("Couldn't mute the sound: {e}");
            remove_marker();
        }
    }
}

/// Unmutes what `mute` muted, if anything.
pub fn restore() {
    let Some(device) = lock().take() else {
        return;
    };
    if let Err(e) = platform::unmute(&device) {
        log::warn!("Couldn't unmute the sound: {e}");
    }
    remove_marker();
}

fn remove_marker() {
    if let Some(marker) = MARKER_PATH.get() {
        let _ = fs::remove_file(marker);
    }
}

fn lock() -> std::sync::MutexGuard<'static, Option<platform::Muted>> {
    MUTED.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(target_os = "macos")]
mod platform {
    //! Core Audio: the mute control of the default output device, on its
    //! main element or, for devices without one, on each channel.

    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;

    type AudioObjectId = u32;
    type OsStatus = i32;

    #[repr(C)]
    struct PropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    const fn code(name: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*name)
    }

    const SYSTEM_OBJECT: AudioObjectId = 1;
    const DEFAULT_OUTPUT_DEVICE: u32 = code(b"dOut");
    const MUTE: u32 = code(b"mute");
    const SCOPE_GLOBAL: u32 = code(b"glob");
    const SCOPE_OUTPUT: u32 = code(b"outp");
    const ELEMENT_MAIN: u32 = 0;
    /// Left and right, for devices that only mute channel by channel.
    const CHANNELS: [u32; 2] = [1, 2];

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectHasProperty(object: AudioObjectId, address: *const PropertyAddress) -> u8;
        fn AudioObjectIsPropertySettable(
            object: AudioObjectId,
            address: *const PropertyAddress,
            settable: *mut u8,
        ) -> OsStatus;
        fn AudioObjectGetPropertyData(
            object: AudioObjectId,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: *mut u32,
            data: *mut c_void,
        ) -> OsStatus;
        fn AudioObjectSetPropertyData(
            object: AudioObjectId,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: u32,
            data: *const c_void,
        ) -> OsStatus;
    }

    pub struct Muted {
        device: AudioObjectId,
        elements: Vec<u32>,
    }

    pub fn mute() -> Result<Option<Muted>, String> {
        let device = default_output()?;
        let elements = mute_elements(device);
        if elements.is_empty() {
            return Err("the output device has no mute control".into());
        }
        if elements.iter().all(|&element| is_muted(device, element)) {
            return Ok(None);
        }
        // Only what this mutes is unmuted afterwards.
        let mut muted = Vec::new();
        for element in elements {
            if !is_muted(device, element) {
                set_mute(device, element, true)?;
                muted.push(element);
            }
        }
        Ok(Some(Muted {
            device,
            elements: muted,
        }))
    }

    pub fn unmute(muted: &Muted) -> Result<(), String> {
        for &element in &muted.elements {
            set_mute(muted.device, element, false)?;
        }
        Ok(())
    }

    pub fn unmute_default() {
        if let Ok(device) = default_output() {
            for element in mute_elements(device) {
                let _ = set_mute(device, element, false);
            }
        }
    }

    fn mute_address(element: u32) -> PropertyAddress {
        PropertyAddress {
            selector: MUTE,
            scope: SCOPE_OUTPUT,
            element,
        }
    }

    fn default_output() -> Result<AudioObjectId, String> {
        let address = PropertyAddress {
            selector: DEFAULT_OUTPUT_DEVICE,
            scope: SCOPE_GLOBAL,
            element: ELEMENT_MAIN,
        };
        let mut device: AudioObjectId = 0;
        let mut size = size_of::<AudioObjectId>() as u32;
        // SAFETY: the address and the out-pointers live for the call, and
        // `size` is that of `device`.
        let status = unsafe {
            AudioObjectGetPropertyData(
                SYSTEM_OBJECT,
                &address,
                0,
                ptr::null(),
                &mut size,
                (&raw mut device).cast(),
            )
        };
        if status != 0 || device == 0 {
            return Err(format!("no default output device (status {status})"));
        }
        Ok(device)
    }

    /// The elements whose mute can be set: the main one, or else each
    /// channel.
    fn mute_elements(device: AudioObjectId) -> Vec<u32> {
        if is_settable(device, ELEMENT_MAIN) {
            return vec![ELEMENT_MAIN];
        }
        CHANNELS
            .into_iter()
            .filter(|&element| is_settable(device, element))
            .collect()
    }

    fn is_settable(device: AudioObjectId, element: u32) -> bool {
        let address = mute_address(element);
        let mut settable = 0u8;
        // SAFETY: the address and the out-pointer live for the calls.
        unsafe {
            AudioObjectHasProperty(device, &address) != 0
                && AudioObjectIsPropertySettable(device, &address, &mut settable) == 0
                && settable != 0
        }
    }

    fn is_muted(device: AudioObjectId, element: u32) -> bool {
        let address = mute_address(element);
        let mut muted: u32 = 0;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: as in `default_output`; the mute control is a UInt32.
        let status = unsafe {
            AudioObjectGetPropertyData(
                device,
                &address,
                0,
                ptr::null(),
                &mut size,
                (&raw mut muted).cast(),
            )
        };
        status == 0 && muted != 0
    }

    fn set_mute(device: AudioObjectId, element: u32, mute: bool) -> Result<(), String> {
        let address = mute_address(element);
        let value = u32::from(mute);
        // SAFETY: the address and the value live for the call, and the size
        // is the value's.
        let status = unsafe {
            AudioObjectSetPropertyData(
                device,
                &address,
                0,
                ptr::null(),
                size_of::<u32>() as u32,
                (&raw const value).cast(),
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!("Core Audio status {status}"))
        }
    }
}

#[cfg(windows)]
mod platform {
    //! The endpoint volume of the default render device. The device is kept
    //! by id: COM objects can't move between threads, and the sound may be
    //! put back from another one.

    use windows::core::{Result as WinResult, HSTRING, PCWSTR};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    pub struct Muted {
        device: String,
    }

    pub fn mute() -> Result<Option<Muted>, String> {
        let attempt = || -> WinResult<Option<Muted>> {
            let device = default_device()?;
            let volume = endpoint_volume(&device)?;
            // SAFETY: plain COM calls on live interfaces.
            unsafe {
                if volume.GetMute()?.as_bool() {
                    return Ok(None);
                }
                let id = device.GetId()?;
                let text = id.to_string();
                CoTaskMemFree(Some(id.0 as *const _));
                volume.SetMute(true, std::ptr::null())?;
                Ok(Some(Muted {
                    device: text.unwrap_or_default(),
                }))
            }
        };
        attempt().map_err(|e| e.to_string())
    }

    pub fn unmute(muted: &Muted) -> Result<(), String> {
        let attempt = || -> WinResult<()> {
            let enumerator = enumerator()?;
            let id = HSTRING::from(muted.device.as_str());
            // SAFETY: plain COM calls on live interfaces; `id` outlives them.
            let device = unsafe { enumerator.GetDevice(PCWSTR(id.as_ptr()))? };
            let volume = endpoint_volume(&device)?;
            unsafe { volume.SetMute(false, std::ptr::null()) }
        };
        attempt().map_err(|e| e.to_string())
    }

    pub fn unmute_default() {
        let attempt = || -> WinResult<()> {
            let volume = endpoint_volume(&default_device()?)?;
            // SAFETY: a plain COM call on a live interface.
            unsafe { volume.SetMute(false, std::ptr::null()) }
        };
        let _ = attempt();
    }

    fn enumerator() -> WinResult<IMMDeviceEnumerator> {
        // SAFETY: COM is set up for this thread (again, harmlessly, if it
        // already was) before the first object is made.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        }
    }

    fn default_device() -> WinResult<IMMDevice> {
        // SAFETY: a plain COM call on a live interface.
        unsafe { enumerator()?.GetDefaultAudioEndpoint(eRender, eConsole) }
    }

    fn endpoint_volume(device: &IMMDevice) -> WinResult<IAudioEndpointVolume> {
        // SAFETY: a plain COM call on a live interface.
        unsafe { device.Activate(CLSCTX_ALL, None) }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    //! PulseAudio, or PipeWire through its PulseAudio server, with `pactl`.

    use std::process::Command;

    pub struct Muted {
        sink: String,
    }

    pub fn mute() -> Result<Option<Muted>, String> {
        let sink = pactl(&["get-default-sink"])?;
        if pactl(&["get-sink-mute", &sink])?.ends_with("yes") {
            return Ok(None);
        }
        pactl(&["set-sink-mute", &sink, "1"])?;
        Ok(Some(Muted { sink }))
    }

    pub fn unmute(muted: &Muted) -> Result<(), String> {
        pactl(&["set-sink-mute", &muted.sink, "0"]).map(drop)
    }

    pub fn unmute_default() {
        let _ = pactl(&["set-sink-mute", "@DEFAULT_SINK@", "0"]);
    }

    fn pactl(args: &[&str]) -> Result<String, String> {
        let output = Command::new("pactl")
            .args(args)
            // "Mute: yes" in English, whatever the desktop's language.
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| format!("pactl: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "pactl {}: {}",
                args[0],
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
mod platform {
    pub struct Muted;

    pub fn mute() -> Result<Option<Muted>, String> {
        Err("not supported on this system".into())
    }

    pub fn unmute(_: &Muted) -> Result<(), String> {
        Ok(())
    }

    pub fn unmute_default() {}
}
