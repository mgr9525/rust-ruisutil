extern crate async_std;
extern crate chrono;
extern crate md5;
extern crate rand;

use async_std::prelude::*;
use rand::{distributions::Standard, prelude::Distribution, Rng};
use std::{
    error,
    io::{self, Read, Write},
    net,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{self, Duration, SystemTime},
};

pub use contianer::ArcMut;
// pub use contianer::ArcMutBox;
pub use timer::Timer;

pub mod bytes;
mod contianer;
pub mod message;
mod timer;

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
pub fn byte2struct<T: Sized>(p: &mut T, bts: &[u8]) -> io::Result<()> {
    let ln = std::mem::size_of::<T>();
    if ln != bts.len() {
        return Err(ioerr("param err!", None));
    }

    unsafe {
        let ptr = p as *mut T as *mut u8;
        let tb = bts.as_ptr();
        std::ptr::copy_nonoverlapping(tb, ptr, ln);
    };
    Ok(())
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

pub async fn tcp_read_async(
    ctx: &Context,
    stream: &mut async_std::net::TcpStream,
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
        match stream.read(&mut data[rn..]).await {
            Ok(n) => {
                if n > 0 {
                    rn += n;
                } else {
                    // let bts=&data[..];
                    // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("read err len:{}!", n),
                    ));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(data.into_boxed_slice())
}
pub async fn tcp_write_async(
    ctx: &Context,
    stream: &mut async_std::net::TcpStream,
    bts: &[u8],
) -> io::Result<usize> {
    if bts.len() <= 0 {
        return Ok(0);
    }
    let mut wn = 0usize;
    while wn < bts.len() {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        match stream.write(&bts[wn..]).await {
            Err(e) => return Err(e),
            Ok(n) => {
                if n > 0 {
                    wn += n;
                } else {
                    // let bts=&data[..];
                    // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                    return Err(io::Error::new(io::ErrorKind::Other, "write err!"));
                }
            }
        }
    }
    Ok(wn)
}

pub fn md5str<S: Into<String>>(input: S) -> String {
    let ms = md5::compute(input.into().as_bytes());
    format!("{:x}", ms)
}
pub fn rands<T>() -> T
where
    Standard: Distribution<T>,
{
    let mut rng = rand::thread_rng();
    rng.gen()
}
pub fn randgs(a: i32, b: i32) -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(a..b)
}
pub fn random(ln: usize) -> String {
    let mut res = String::new();
    if ln <= 0 {
        return res;
    }
    let mut rng = rand::thread_rng();
    const BS: &[u8] = b"0123456789AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz";
    for _ in 0..ln {
        let i = rng.gen_range(0..BS.len());
        res.push(BS[i] as char);
    }
    res
}
pub fn strftime<T>(dt: T, s: &str) -> String
where
    T: Into<chrono::DateTime<chrono::Local>>,
{
    format!("{}", dt.into().format(s))
}
pub fn strftime_utc<T>(dt: T, s: &str) -> String
where
    T: Into<chrono::DateTime<chrono::Utc>>,
{
    format!("{}", dt.into().format(s))
}
pub fn strptime(t: &str,s: &str) -> io::Result<SystemTime> {
    let date = match chrono::DateTime::parse_from_str(t,s) {
        Err(e) => return Err(crate::ioerr(format!("parse err:{}", e), None)),
        Ok(v) => v,
    };
    let tm = SystemTime::from(date);
    Ok(tm)
}

#[derive(Clone)]
pub struct Context {
    inner: Arc<CtxInner>,
}
struct CtxInner {
    parent: Option<Context>,
    doned: Mutex<bool>,

    times: time::SystemTime,
    timeout: Mutex<Option<time::Duration>>,
}

impl Context {
    pub fn background(prt: Option<Context>) -> Self {
        Self {
            inner: Arc::new(CtxInner {
                parent: prt,
                doned: Mutex::new(false),
                times: time::SystemTime::now(),
                timeout: Mutex::new(None),
            }),
        }
    }

    pub fn with_timeout(prt: Option<Context>, tmd: time::Duration) -> Self {
        let c = Self::background(prt);
        if let Ok(mut v) = c.inner.timeout.lock() {
            *v = Some(tmd)
        }
        c
    }

    pub fn done(&self) -> bool {
        if let Some(v) = &self.inner.parent {
            if v.done() {
                return true;
            }
        };
        if let Some(v) = &*self.inner.timeout.lock().unwrap() {
            if let Ok(vs) = time::SystemTime::now().duration_since(self.inner.times) {
                if vs.gt(v) {
                    return true;
                }
            }
        }
        *self.inner.doned.lock().unwrap()
    }

    pub fn stop(&self) -> bool {
        match self.inner.doned.lock() {
            Err(_) => false,
            Ok(mut v) => {
                *v = true;
                true
            }
        }
    }
}

pub struct WaitGroup {
    inner: Arc<WgInner>,
}

/// Inner state of a `WaitGroup`.
struct WgInner {
    count: Mutex<usize>,
}
impl WaitGroup {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WgInner {
                count: Mutex::new(1),
            }),
        }
    }
    pub fn wait(&self) {
        loop {
            std::thread::sleep(Duration::from_millis(1));
            let count = self.inner.count.lock().unwrap();
            if *count <= 1 {
                break;
            }
        }
    }
    pub async fn waits(&self) {
        loop {
            async_std::task::sleep(Duration::from_millis(1)).await;
            let count = self.inner.count.lock().unwrap();
            if *count <= 1 {
                break;
            }
        }
    }
}
impl Drop for WaitGroup {
    fn drop(&mut self) {
        if let Ok(mut v) = self.inner.count.lock() {
            *v -= 1;
        }
    }
}

impl Clone for WaitGroup {
    fn clone(&self) -> WaitGroup {
        if let Ok(mut v) = self.inner.count.lock() {
            *v += 1;
        }

        WaitGroup {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ArcMut;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    struct Inner {
        i: i32,
    }
    #[derive(Clone)]
    struct Ruis {
        inner: ArcMut<Inner>,
    }
    impl Ruis {
        pub fn new() -> Self {
            Self {
                inner: ArcMut::new(Inner { i: 1 }),
            }
        }

        pub fn set(&self, i: i32) {
            let ins = unsafe { self.inner.muts() };
            ins.i = i;
        }
        pub fn get(&self) -> i32 {
            self.inner.i
        }
    }

    #[test]
    fn contias() {
        let ruis = Ruis::new();
        println!("ruis i-1:{}", ruis.get());
        ruis.set(2);
        println!("ruis i-2:{}", ruis.get());
        ruis.set(3);
        let ruis1 = ruis.clone();
        ruis1.set(4);
        println!("ruis i-3:{}", ruis.get());
    }

    #[test]
    fn rands() {
        println!("randoms:{}", crate::random(32));
    }

    #[test]
    fn md5s() {
        println!(
            "md5s:{}",
            crate::md5str("ahsdhflasjdklfjalskdjflksdjlfkjslkdjf")
        );
    }

    #[test]
    fn tms() {
        let now = std::time::SystemTime::now();
        println!("{}", crate::strftime(now.clone(), "%+"));
        println!("{}", crate::strftime(now.clone(), "%Y-%m-%d %H:%M:%S"));
        match crate::strptime("2022-02-10T15:09:12.309627600+08:00","%+") {
            Err(e) => println!("strptime err:{}", e),
            Ok(v) => println!("parse:{}", crate::strftime(v.clone(), "%+")),
        }
    }
}
