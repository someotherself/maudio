#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![no_std]

#[cfg(feature = "generate-bindings")]
pub mod ffi {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[cfg(not(feature = "generate-bindings"))]
pub mod ffi {
    #[cfg(unix)]
    include!("pregen_bindings/unix.rs");

    #[cfg(windows)]
    include!("pregen_bindings/windows.rs");
}
