pub mod zip;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm;
