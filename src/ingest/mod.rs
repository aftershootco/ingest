#[cfg(all(feature = "async", not(feature = "sync")))]
mod r#async;
#[cfg(all(feature = "async", not(feature = "sync")))]
pub use r#async::*;

#[cfg(all(feature = "sync", not(feature = "async")))]
mod sync;
#[cfg(all(feature = "sync", not(feature = "async")))]
pub use sync::*;

#[cfg(all(feature = "sync", feature = "async"))]
compile_error!("Can't compile for both async and sync");
