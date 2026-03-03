#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn main() {
    palyra_desktop_control_center::run();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn main() {
    panic!(
        "Desktop Control Center currently supports Windows/macOS only (Linux runtime temporarily disabled due upstream glib advisory constraints)"
    );
}
