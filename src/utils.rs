#[cfg(any(feature = "asyncs", feature = "tokios"))]
use crate::asyncs::{self, AsyncReadExt, AsyncWriteExt};
#[cfg(feature = "chrono")]
use chrono::TimeZone;
use std::{
    error,
    io::{self, Read, Write},
    net,
    str::FromStr,
    time::{Duration, SystemTime},
};

use crate::Context;

pub fn byte_2i(bts: &[u8]) -> i64 {
    let mut rt = 0i64;
    let mut i = bts.len();
    for v in bts {
        if i > 0 {
            i -= 1;
            rt |= (*v as i64) << (8 * i);
        } else {
            rt |= *v as i64;
        }
    }
    rt
}

pub fn i2_byte(v: i64, n: usize) -> Box<[u8]> {
    let mut rt: Vec<u8> = Vec::with_capacity(n);
    // if n>4{return rt;}
    for i in 0..n {
        let k = n - i - 1;
        if k > 0 {
            rt.push((v >> (8 * k)) as u8);
        } else {
            rt.push(v as u8)
        }
    }
    rt.into_boxed_slice()
}

pub fn ioerr<E>(s: E, kd: Option<io::ErrorKind>) -> io::Error
where
    E: Into<Box<dyn error::Error + Send + Sync>>,
{
    let mut kds = io::ErrorKind::Other;
    if let Some(v) = kd {
        kds = v;
    }
    io::Error::new(kds, s)
}
pub fn struct2byte<T: Sized>(p: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((p as *const T) as *const u8, std::mem::size_of::<T>()) }
}
pub fn byte2struct<T: Sized>(p: &mut T, bts: &[u8]) -> io::Result<usize> {
    let ln = std::mem::size_of::<T>();
    if ln > bts.len() {
        return Err(ioerr("param err!", None));
    }

    unsafe {
        let ptr = p as *mut T as *mut u8;
        let tb = (&bts[..ln]).as_ptr();
        std::ptr::copy_nonoverlapping(tb, ptr, ln);
    };
    Ok(ln)
}

pub fn parse_noip_addr<T: AsRef<str>>(s: T) -> String {
    let idx = s.as_ref().rfind(':');
    match idx {
        Some(i) => {
            if i == 0 {
                format!("0.0.0.0{}", &s.as_ref()[i..])
            // }else if s.as_ref().contains("[")||s.as_ref().contains("::")||s.as_ref().contains("]"){
            } else {
                s.as_ref().to_string()
            }
        }
        None => format!("{}:0", s.as_ref()),
    }
}
pub fn tcp_read(ctx: &Context, stream: &mut net::TcpStream, ln: usize) -> io::Result<Box<[u8]>> {
    if ln <= 0 {
        return Ok(Box::new([0u8; 0]));
    }
    let mut rn = 0usize;
    let mut data = vec![0u8; ln];
    while rn < ln {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        match stream.read(&mut data[rn..]) {
            Ok(n) => {
                if n > 0 {
                    rn += n;
                } else {
                    // let bts=&data[..];
                    // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                    return Err(io::Error::new(io::ErrorKind::Other, "read err!"));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(data.into_boxed_slice())
}
pub fn tcp_write(ctx: &Context, stream: &mut net::TcpStream, bts: &[u8]) -> io::Result<usize> {
    if bts.len() <= 0 {
        return Ok(0);
    }
    if ctx.done() {
        return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
    }
    match stream.write(bts) {
        Err(e) => Err(e),
        Ok(n) => {
            if n != bts.len() {
                Err(ioerr(format!("send len err:{}/{}", n, bts.len()), None))
            } else {
                Ok(n)
            }
        }
    }
}

#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn fut_tmout_ctxend0<F, T>(ctx: &Context, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>>,
{
    fut_tmout_ctxends(ctx, Duration::from_secs(3), Box::pin(future)).await
}
#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn fut_tmout_ctxend0s<F, T>(ctx: &Context, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>> + Unpin,
{
    fut_tmout_ctxends(ctx, Duration::from_secs(3), future).await
}
#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn fut_tmout_ctxend<F, T>(ctx: &Context, mut secs: u64, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>> + Unpin,
{
    if secs < 3 {
        secs = 3;
    }
    fut_tmout_ctxends(ctx, Duration::from_secs(secs), future).await
}
#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn fut_tmout_ctxends<F, T>(
    ctx: &Context,
    mut drt: Duration,
    mut future: F,
) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>> + Unpin,
{
    if drt < Duration::from_millis(10) {
        drt = Duration::from_millis(10);
    }
    while !ctx.done() {
        match timeoutios(drt, &mut future).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::TimedOut {
                    return Err(e);
                }
            }
        }
    }
    Err(crate::ioerr(
        "ctx end",
        Some(std::io::ErrorKind::Interrupted),
    ))
}

#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn timeoutios<F, T>(duration: std::time::Duration, future: F) -> std::io::Result<T>
where
    F: core::future::Future<Output = std::io::Result<T>>,
{
    crate::asyncs::timeouts(duration, future).await
}

use crate::bytes;
use std::sync::Arc;

#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn read_allbuf_async<T: asyncs::AsyncReadExt + Unpin>(
    ctx: &Context,
    stream: &mut T,
    mut eln: usize,
) -> io::Result<bytes::ByteBoxBuf> {
    let mut buf = bytes::ByteBoxBuf::new();
    if eln <= 0 {
        eln = 1024 * 5;
    }
    while buf.len() < eln {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        let mut data = vec![0u8; eln].into_boxed_slice();
        let n = fut_tmout_ctxend0(ctx, stream.read(&mut data[..])).await?;
        if n <= 0 {
            break;
        }
        buf.pushs(Arc::new(data), 0, n);
    }

    Ok(buf)
}

#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn read_all_async<T: asyncs::AsyncReadExt + Unpin>(
    ctx: &Context,
    stream: &mut T,
    ln: usize,
) -> io::Result<Box<[u8]>> {
    if ln <= 0 {
        return Ok(Box::new([0u8; 0]));
    }
    let mut rn = 0usize;
    let mut data = vec![0u8; ln];
    while rn < ln {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        match fut_tmout_ctxend0(ctx, stream.read(&mut data[rn..])).await {
            Ok(n) => {
                if n > 0 {
                    rn += n;
                } else {
                    // let bts=&data[..];
                    // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("read err len:{}!", n),
                    ));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(data.into_boxed_slice())
}

#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn write_all_async<T: asyncs::AsyncWriteExt + Unpin>(
    ctx: &Context,
    stream: &mut T,
    bts: &[u8],
) -> io::Result<usize> {
    let sz = bts.len();
    if sz <= 0 {
        return Ok(0);
    }
    let mut wn = 0usize;
    while wn < sz {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        let n = fut_tmout_ctxend0(ctx, stream.write(&bts[wn..])).await?;
        if n > 0 {
            wn += n;
        } else {
            // let bts=&data[..];
            // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
            return Err(io::Error::new(io::ErrorKind::Other, "write err!"));
        }
    }
    stream.flush().await?;
    Ok(wn)
}
#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub async fn write_allbuf_async<T: asyncs::AsyncWriteExt + Unpin>(
    ctx: &Context,
    stream: &mut T,
    bts: &bytes::ByteBoxBuf,
) -> io::Result<usize> {
    let sz = bts.len();
    if sz <= 0 {
        return Ok(0);
    }
    let mut wn = 0usize;
    let its = bts.iter();
    for v in its {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        wn += write_all_async(ctx, stream, &v[..]).await?
    }
    Ok(wn)
}

pub fn read_allbuf<T: std::io::Read>(
    ctx: &Context,
    stream: &mut T,
    mut eln: usize,
) -> io::Result<bytes::ByteBoxBuf> {
    let mut buf = bytes::ByteBoxBuf::new();
    if eln <= 0 {
        eln = 1024 * 5;
    }
    loop {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        let mut data = vec![0u8; eln].into_boxed_slice();
        let n = stream.read(&mut data[..])?;
        if n <= 0 {
            break;
        }
        buf.pushs(Arc::new(data), 0, n);
    }

    Ok(buf)
}
pub fn read_all<T: std::io::Read>(
    ctx: &Context,
    stream: &mut T,
    ln: usize,
) -> io::Result<Box<[u8]>> {
    if ln <= 0 {
        return Ok(Box::new([0u8; 0]));
    }
    let mut rn = 0usize;
    let mut data = vec![0u8; ln];
    while rn < ln {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        match stream.read(&mut data[rn..]) {
            Ok(n) => {
                if n > 0 {
                    rn += n;
                } else {
                    // let bts=&data[..];
                    // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("read err len:{}!", n),
                    ));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(data.into_boxed_slice())
}
pub fn write_all<T: std::io::Write>(
    ctx: &Context,
    stream: &mut T,
    bts: &[u8],
) -> io::Result<usize> {
    let sz = bts.len();
    if sz <= 0 {
        return Ok(0);
    }
    let mut wn = 0usize;
    while wn < sz {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        let n = stream.write(&bts[wn..])?;
        if n > 0 {
            wn += n;
        } else {
            // let bts=&data[..];
            // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
            return Err(io::Error::new(io::ErrorKind::Other, "write err!"));
        }
    }
    stream.flush()?;
    Ok(wn)
}
pub fn write_allbuf<T: std::io::Write>(
    ctx: &Context,
    stream: &mut T,
    bts: &bytes::ByteBoxBuf,
) -> io::Result<usize> {
    let sz = bts.len();
    if sz <= 0 {
        return Ok(0);
    }
    let mut wn = 0usize;
    let its = bts.iter();
    for v in its {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        wn += write_all(ctx, stream, &v[..])?
    }
    Ok(wn)
}

pub fn env(key: &str) -> Option<String> {
    match std::env::var(key) {
        Err(_) => None,
        Ok(v) => Some(v),
    }
}
pub fn envs(key: &str, defs: &str) -> String {
    match env(key) {
        None => String::from(defs),
        Some(vs) => {
            if vs.is_empty() {
                String::from(defs)
            } else {
                vs
            }
        }
    }
}
pub fn envi(key: &str, defs: i64) -> i64 {
    match env(key) {
        None => defs,
        Some(vs) => match vs.parse::<i64>() {
            Ok(v) => v,
            Err(_) => defs,
        },
    }
}

pub fn print_hex(data: &[u8]) {
    if data.len() <= 0 {
        return;
    }
    print!("{:02x}", data[0]);
    for i in 1..data.len() {
        print!(" {:02x}", data[i]);
    }
}
pub fn sprint_hex(data: &[u8], splts: &str) -> String {
    let mut rts = String::new();
    if data.len() > 0 {
        rts += format!("{:02x}", data[0]).as_str();
        for i in 1..data.len() {
            rts += format!("{}{:02x}", splts, data[i]).as_str();
        }
    }
    rts
}
pub fn sprints_hex(data: &[u8], mut ln: usize, splts: &str) -> String {
    let mut rts = String::new();
    if data.len() > 0 {
        if ln <= 0 || ln > data.len() {
            ln = data.len();
        }
        rts += format!("{:02x}", data[0]).as_str();
        for i in 1..ln {
            rts += format!("{}{:02x}", splts, data[i]).as_str();
        }
    }
    rts
}
pub fn md5str<S: Into<String>>(input: S) -> String {
    let ms = md5::compute(input.into().as_bytes());
    format!("{:x}", ms)
}
pub fn md5strs<S: AsRef<[u8]>>(input: S) -> String {
    let ms = md5::compute(input);
    format!("{:x}", ms)
}
#[cfg(feature = "sha")]
pub use crypto::digest::Digest as CryptoDigest;
#[cfg(feature = "sha")]
pub use crypto::sha1::Sha1 as CryptoSha1;
#[cfg(feature = "sha")]
pub use crypto::sha2::Sha256 as CryptoSha256;
/* #[cfg(feature = "sha")]
pub fn sha1str<S: Into<String>>(input: S) -> String {
    let mut hld = crypto::sha1::Sha1::new();
    hld.input(input.into().as_bytes());
    hld.result_str()
} */
#[cfg(feature = "sha")]
pub fn sha1str<S: AsRef<[u8]>>(input: S) -> String {
    let mut hld = crypto::sha1::Sha1::new();
    hld.input(input.as_ref());
    hld.result_str()
}
/* #[cfg(feature = "sha")]
pub fn sha256str<S: Into<String>>(input: S) -> String {
    let mut hld = crypto::sha2::Sha256::new();
    hld.input(input.into().as_bytes());
    hld.result_str()
} */
#[cfg(feature = "sha")]
pub fn sha256str<S: AsRef<[u8]>>(input: S) -> String {
    let mut hld = crypto::sha2::Sha256::new();
    hld.input(input.as_ref());
    hld.result_str()
}

pub fn times() -> (Duration, i8) {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => (n, 1),
        Err(_) => match SystemTime::UNIX_EPOCH.duration_since(SystemTime::now()) {
            Ok(n) => (n, 0),
            Err(_) => (Duration::default(), -1),
        },
    }
}
pub fn tms_since(tm_new: SystemTime, tm_old: SystemTime) -> Duration {
    match tm_new.duration_since(tm_old) {
        Ok(n) => n,
        Err(_) => Duration::default(),
    }
}
pub fn tms_now_since(tm_old: SystemTime) -> Duration {
    tms_since(SystemTime::now(), tm_old)
}
pub fn randtms() -> u32 {
    let (tms, _) = times();
    (tms.as_nanos() & 0xffffffff) as u32
}

pub fn randtms_ang(a: u32, b: u32) -> u32 {
    if b <= 0 || a > b {
        return 0;
    }
    let mut rds = randtms();
    if rds > b {
        rds %= b;
    }
    while rds != 0 {
        if rds > b {
            rds /= 2;
        } else if rds < a {
            rds = rds * 2 + 1;
        } else {
            break;
        }
    }
    rds
}
/* pub fn rands<T>() -> T
where
    Standard: Distribution<T>,
{
    let mut rng = rand::thread_rng();
    rng.gen()
}
pub fn randgs(a: i32, b: i32) -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(a..b)
} */
pub fn randoms() -> String {
    randtms().to_string()
}
pub fn random(ln: usize) -> String {
    let mut res = String::new();
    if ln <= 0 {
        return res;
    }
    const BS: &[u8] = b"0123456789AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz";
    for _ in 0..ln {
        let i = randtms_ang(0, BS.len() as u32);
        res.push(BS[i as usize] as char);
    }
    res
}
#[cfg(feature = "chrono")]
pub fn strftime<T>(dt: T, s: &str) -> String
where
    T: Into<chrono::DateTime<chrono::Local>>,
{
    format!("{}", dt.into().format(s))
}
#[cfg(feature = "chrono")]
pub fn strftime_off<T>(dt: T, s: &str, hour: i32) -> String
where
    T: Into<chrono::DateTime<chrono::Utc>>,
{
    if let Some(v) = chrono::FixedOffset::east_opt(hour * 3600) {
        let tm: chrono::DateTime<chrono::Utc> = dt.into();
        let tme = tm.with_timezone(&v);
        format!("{}", tme.format(s))
    } else {
        "ErrHour".to_string()
    }
}
#[cfg(feature = "chrono")]
pub fn strftime_utc<T>(dt: T, s: &str) -> String
where
    T: Into<chrono::DateTime<chrono::Utc>>,
{
    format!("{}", dt.into().format(s))
}
#[cfg(feature = "chrono")]
pub fn strptime(t: &str, s: &str) -> io::Result<SystemTime> {
    match chrono::DateTime::parse_from_str(t, s) {
        Ok(v) => Ok(SystemTime::from(v)),
        Err(e) => Err(crate::ioerr(format!("parse {} err:{}", t, e), None)),
    }
}
#[cfg(feature = "chrono")]
pub fn strptime_off(t: &str, s: &str, hour: i32) -> io::Result<SystemTime> {
    match chrono::FixedOffset::east_opt(hour * 3600) {
        None => Err(crate::ioerr(format!("timezone offset {} err", hour), None)),
        Some(fot) => match chrono::DateTime::parse_from_str(t, s) {
            Ok(v) => Ok(SystemTime::from(v)),
            Err(_) => match chrono::NaiveDateTime::parse_from_str(t, s) {
                Ok(nvt) => {
                    let tme = match fot.from_local_datetime(&nvt) {
                        chrono::LocalResult::None => {
                            return Err(crate::ioerr("local tm nil", None))
                        }
                        chrono::LocalResult::Single(v) => v,
                        chrono::LocalResult::Ambiguous(v, e) => {
                            return Err(crate::ioerr("local tm err", None))
                        }
                    };
                    Ok(SystemTime::from(tme))
                }
                Err(e) => Err(crate::ioerr(format!("parse {} err:{}", t, e), None)),
            },
        },
    }
}
