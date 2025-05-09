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
impl<T: Future> FutureExt for T {}

/* pub struct ShutdownwFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
}

impl<T: AsyncWrite + Unpin + ?Sized> Future for ShutdownwFuture<'_, T> {
    type Output = std::io::Result<()>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::pin::pin!(&mut *self.writer).poll_close(cx)
    }
} */
pub trait IO: AsyncRead + AsyncWrite + Unpin {
    fn shutdownw<'a>(&'a mut self) -> BoxFuture<'a, std::io::Result<()>>;
}
impl IO for net::TcpStream {
    fn shutdownw<'a>(&'a mut self) -> BoxFuture<'a, std::io::Result<()>> {
        async move {
            #[cfg(feature = "asyncs")]
            {
                self.shutdown(std::net::Shutdown::Write);
                Ok(())
            }
            #[cfg(feature = "tokios")]
            {
                self.shutdown().await
            }
        }
        .boxed()
    }
}
impl IO for fs::File {
    fn shutdownw<'a>(&'a mut self) -> BoxFuture<'a, std::io::Result<()>> {
        async move { self.flush().await }.boxed()
    }
}

pub async fn waitctx(ctx: &crate::Context) {
    while !ctx.done() {
        sleep(std::time::Duration::from_millis(50)).await;
    }
}
pub async fn waitctxs(ctx: &crate::Context, tms: std::time::Duration) {
    let cx = crate::Context::with_timeout(Some(ctx.clone()), tms);
    waitctx(&cx).await
}
