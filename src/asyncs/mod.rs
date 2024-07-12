




#[cfg(feature = "asyncs")]
mod stds;
#[cfg(feature = "tokios")]
mod tkos;

#[cfg(feature = "asyncs")]
pub use stds::*;
#[cfg(feature = "tokios")]
pub use tkos::*;

pub use core::future::Future;
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn core::future::Future<Output = T> + Send + 'a>>;

