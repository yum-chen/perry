//! Common utilities for stdlib modules

pub mod handle;
// Tokio-backed promise/runtime bridge — only needed when an async feature
// (http-server/client, websocket, databases, email, scheduler, rate-limit,
// crypto's bcrypt path, …) pulls in `async-runtime`. Always-on code that
// references it must also be `#[cfg(feature = "async-runtime")]`-gated.
#[cfg(feature = "async-runtime")]
pub mod async_bridge;
pub mod dispatch;

pub use handle::*;
#[cfg(feature = "async-runtime")]
pub use async_bridge::*;
pub use dispatch::*;

#[no_mangle]
pub extern "C" fn js_stdlib_to_bool(v: f64) -> i32 {
    if perry_runtime::JSValue::from_bits(v.to_bits()).to_bool() {
        1
    } else {
        0
    }
}
