#[cfg(feature = "tokios")]
pub use tokio;

pub use tokio::fs;
pub use tokio::io;
pub use tokio::net;
pub use tokio::sync;
pub use tokio::task;

pub use tokio::sync::mpsc::{Receiver, Sender};
pub use tokio::time::timeout;

pub use tokio::io::AsyncRead;
pub use tokio::io::AsyncReadExt;
pub use tokio::io::AsyncWrite;
pub use tokio::io::AsyncWriteExt;

pub fn is_async_std() -> bool {
    false
}
pub fn block_on<F>(future: F) -> std::io::Result<()>
where
    F: core::future::Future<Output = std::io::Result<()>>,
{
    let rtm = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rtm.block_on(future)
}
pub fn current_block_on<F: core::future::Future>(future: F) -> std::io::Result<F::Output> {
    let rtm = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rtm.block_on(future))
}
pub async fn spawn_blocking_io<F, T>(f: F) -> std::io::Result<T>
where
    F: FnOnce() -> std::io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    match task::spawn_blocking(f).await {
        Ok(v) => v,
        Err(e) => Err(crate::ioerr("tokio join err", None)),
    }
}
pub async fn sleep(dur: std::time::Duration) {
    tokio::time::sleep(dur).await
}
pub async fn timeouts<F, T, E>(duration: std::time::Duration, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    match timeout(duration, future).await {
        Ok(v) => match v {
            Ok(v) => Ok(v),
            Err(e) => {
                match e.source() {
                    Some(v) => match v.downcast_ref::<std::io::Error>() {
                        Some(ioe) => Err(crate::ioerr(format!("{}", ioe), Some(ioe.kind()))),
                        None => Err(crate::ioerr(format!("other err found:{}", e), None)),
                    },
                    None => Err(crate::ioerr("not err found", None)),
                }
                /* if let Some(ioe) = e.source().and_then(|v| v.downcast_ref::<std::io::Error>()) {
                    Err(std::io::Error::new(ioe.kind(), ioe))
                } else {
                    Err(crate::ioerr("not err found", None))
                } */
            }
        },
        Err(e) => Err(crate::ioerr(
            "future timed out",
            Some(std::io::ErrorKind::TimedOut),
        )),
    }
}
/* pub fn tcp_shutdownw(conn: &mut net::TcpStream) -> std::io::Result<()> {
    let rtm = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rtm.block_on(async { conn.shutdown().await })
} */
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
