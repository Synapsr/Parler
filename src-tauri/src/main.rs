// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Windows: overlays and capture tools hook every Vulkan app through
    // implicit layers, and some crash it. Off for Parler's transcription,
    // unless asked otherwise; set before any thread or driver starts.
    #[cfg(windows)]
    if std::env::var_os("VK_LOADER_LAYERS_DISABLE").is_none()
        && std::env::var_os("PARLER_KEEP_VULKAN_LAYERS").is_none()
    {
        std::env::set_var("VK_LOADER_LAYERS_DISABLE", "~implicit~");
    }
    parler_lib::run();
}
