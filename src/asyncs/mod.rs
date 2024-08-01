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

pub trait FutureExt: Future {
    fn boxed<'a>(self) -> BoxFuture<'a, Self::Output>
    where
        Self: Sized + Send + 'a,
    {
        Box::pin(self)
    }
}
impl<F: Future> FutureExt for F {}

pub trait IO: AsyncRead + AsyncWrite + Unpin {}
impl IO for net::TcpStream {}
impl IO for fs::File {}
