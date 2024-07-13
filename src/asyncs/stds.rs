#[cfg(feature = "asyncs")]
pub use async_std;

pub use async_std::fs;
pub use async_std::net;
pub use async_std::sync;
pub use async_std::task;

pub use async_std::channel::{Receiver, Sender};
pub use async_std::io::timeout;

pub use async_std::io::ReadExt as AsyncReadExt;
pub use async_std::io::WriteExt as AsyncWriteExt;

pub fn block_on<F: core::future::Future>(future: F) -> std::io::Result<F::Output> {
    Ok(task::block_on(future))
}
pub fn current_block_on<F: core::future::Future>(future: F) -> std::io::Result<F::Output> {
    Ok(task::block_on(future))
}
pub async fn spawn_blocking_io<F, T>(f: F) -> std::io::Result<T>
where
    F: FnOnce() -> std::io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    task::spawn_blocking(f).await
}
pub async fn sleep(dur: std::time::Duration) {
    async_std::task::sleep(dur).await
}
pub async fn timeouts<F, T>(duration: std::time::Duration, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>>,
{
    /* match timeout(duration, future).await {
        Ok(v) => v,
        Err(e) => Err(crate::ioerr("timeout", Some(std::io::ErrorKind::TimedOut))),
    } */
    timeout(duration, future).await
}

/* pub fn tcp_shutdownw(conn: &mut net::TcpStream) -> std::io::Result<()> {
    conn.shutdown(std::net::Shutdown::Write)
} */
pub async fn tcp_shutdownw_ac(conn: &mut net::TcpStream) -> std::io::Result<()> {
    conn.shutdown(std::net::Shutdown::Write)
}

pub fn make_channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    async_std::channel::bounded(buffer)
}
pub fn close_channel_snd<T>(snd: &Sender<T>) -> bool {
    snd.close()
}

pub async fn channel_recv<T>(rcv: &mut Receiver<T>) -> std::io::Result<T> {
    match rcv.recv().await {
        Ok(v) => Ok(v),
        Err(e) => Err(crate::ioerr("nil", Some(std::io::ErrorKind::InvalidData))),
    }
}
