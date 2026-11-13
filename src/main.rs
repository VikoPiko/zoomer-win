#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod graphics;
#[cfg(windows)]
mod platform;

fn main() {
    #[cfg(windows)]
    if let Err(error) = app::run() {
        platform::message(&format!("Zoomer could not start: {error}"));
    }
    #[cfg(not(windows))]
    eprintln!("Zoomer currently supports Windows 10/11 x64 only.");
}
