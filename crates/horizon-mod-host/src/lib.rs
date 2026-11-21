//! Installed mod code loading above the engine (docs/layers.md, arismoko/horizon#2534).
//! The platform prepares the engine, loads mods here, then makes one engine start call.
#![deny(unsafe_code)]

#[cfg(not(target_os = "horizon"))]
#[allow(unsafe_code)]
mod native_libraries;
#[cfg(not(target_os = "horizon"))]
pub use native_libraries::{GAME_BUILD_ID, RUSTC_VERSION, main};

mod loading;
pub use loading::main_with;

/// Establish main-thread stack capacity before compiler-child handling.
pub use horizon_guest_stack::prepare_main as prepare_main_stack;
#[cfg(not(target_os = "horizon"))]
pub use horizon_mod_wasm::compile_child;

/// Handles the private compiler invocation before any game startup work.
pub fn exit_if_compile_child() {
    horizon_guest_stack::prepare_main();
    // The Switch compiles no wasm (and starts no child process).
    #[cfg(not(target_os = "horizon"))]
    if let Some(code) = compile_child(std::env::args_os().skip(1)) {
        std::process::exit(code);
    }
}

/// A typed API-point listener supplied by generated native startup.
pub use mod_points::__private::SlotListener;
