#[cfg(feature = "tokios")]
pub use tokio;

pub use tokio::net;
pub use tokio::sync;
pub use tokio::task;

pub use tokio::sync::mpsc::{Receiver, Sender};
pub use tokio::time::timeout;

pub use tokio::io::AsyncReadExt;
pub use tokio::io::AsyncWriteExt;

pub fn current_block_on<F: core::future::Future>(future: F) -> std::io::Result<F::Output> {
    let rtm = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rtm.block_on(future))
}
pub async fn sleep(dur: std::time::Duration) {
    tokio::time::sleep(dur).await
}
pub async fn timeouts<F, T>(duration: std::time::Duration, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>>,
{
    match timeout(duration, future).await {
        Ok(v) => v,
        Err(e) => Err(crate::ioerr(
            "future timed out",
            Some(std::io::ErrorKind::TimedOut),
        )),
    }
}
pub fn tcp_shutdownw(conn: &mut net::TcpStream) -> std::io::Result<()> {
    let rtm = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rtm.block_on(async { conn.shutdown().await })
}
pub async fn tcp_shutdownw_ac(conn: &mut net::TcpStream) -> std::io::Result<()> {
    conn.shutdown().await
}

pub fn make_channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    tokio::sync::mpsc::channel(buffer)
}
pub fn close_channel_snd<T>(snd: &Sender<T>) -> bool {
    snd.is_closed()
}

pub async fn channel_recv<T>(rcv: &mut Receiver<T>) -> std::io::Result<T> {
    match rcv.recv().await {
        Some(v) => Ok(v),
        None => Err(crate::ioerr("nil", Some(std::io::ErrorKind::InvalidData))),
    }
}
