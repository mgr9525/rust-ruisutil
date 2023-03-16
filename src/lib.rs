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
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc,
    },
    time::{self, Duration, SystemTime},
};

pub use contianer::ArcMut;
// pub use contianer::ArcMutBox;
pub use timer::Timer;

pub mod bytes;
mod contianer;
pub mod message;
pub mod sync;
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

pub fn env(key: &str) -> Option<String> {
    match std::env::var(key) {
        Err(_) => None,
        Ok(v) => Some(v),
    }
}
pub fn envs(key: &str, defs: &str) -> String {
    match std::env::var(key) {
        Err(_) => String::from(defs),
        Ok(vs) => {
            if vs.is_empty() {
                String::from(defs)
            } else {
                vs
            }
        }
    }
}

pub fn print_hex(data: &[u8]) {
    if data.len() <= 0 {
        return;
    }
    print!("{:x}", data[0]);
    for i in 1..data.len() {
        print!(" {:x}", data[i]);
    }
}
pub fn sprint_hex(data: &[u8]) -> String {
    let mut rts = String::new();
    if data.len() > 0 {
        for i in 0..data.len() {
            rts += format!("{:x}", data[i]).as_str();
        }
    }
    rts
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
pub fn strptime(t: &str, s: &str) -> io::Result<SystemTime> {
    let date = match chrono::DateTime::parse_from_str(t, s) {
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
    doned: AtomicBool,

    times: time::SystemTime,
    timeout: Option<time::Duration>,
}

impl CtxInner {
    fn new(prt: Option<Context>) -> Self {
        Self {
            parent: prt,
            doned: AtomicBool::new(false),
            times: time::SystemTime::now(),
            timeout: None,
        }
    }
}
impl Context {
    pub fn background(prt: Option<Context>) -> Self {
        Self {
            inner: Arc::new(CtxInner::new(prt)),
        }
    }

    pub fn with_timeout(prt: Option<Context>, tmd: time::Duration) -> Self {
        let mut inr = CtxInner::new(prt);
        inr.timeout = Some(tmd);
        Self {
            inner: Arc::new(inr),
        }
    }

    pub fn done(&self) -> bool {
        if let Some(v) = &self.inner.parent {
            if v.done() {
                return true;
            }
        };
        if let Some(v) = &self.inner.timeout {
            if let Ok(vs) = time::SystemTime::now().duration_since(self.inner.times) {
                if vs.gt(v) {
                    return true;
                }
            }
        }
        self.inner.doned.load(Ordering::SeqCst)
    }

    pub fn stop(&self) -> bool {
        self.inner.doned.store(true, Ordering::SeqCst);
        true
    }
}

pub struct WaitGroup {
    inner: Arc<WgInner>,
}

/// Inner state of a `WaitGroup`.
struct WgInner {
    count: AtomicI32,
}
impl WaitGroup {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WgInner {
                count: AtomicI32::new(0),
            }),
        }
    }
    pub fn wait(&self) {
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if self.done() {
                break;
            }
        }
    }
    pub async fn waits(&self) {
        loop {
            async_std::task::sleep(Duration::from_millis(100)).await;
            if self.done() {
                break;
            }
        }
    }
    pub fn done(&self) -> bool {
        let count = self.inner.count.load(Ordering::SeqCst);
        if count <= 0 {
            true
        } else {
            false
        }
    }
}
impl Drop for WaitGroup {
    fn drop(&mut self) {
        self.inner.count.fetch_add(-1, Ordering::SeqCst);
    }
}

impl Clone for WaitGroup {
    fn clone(&self) -> WaitGroup {
        self.inner.count.fetch_add(1, Ordering::SeqCst);
        WaitGroup {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_std::task;

    use crate::{bytes::CircleBuf, ArcMut, Context};

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

        pub unsafe fn from_raw(p: *const Inner) -> std::io::Result<Self> {
            let inr = ArcMut::from_raw(p)?;
            Ok(Self { inner: inr })
        }

        pub unsafe fn from_raws(p: *const Inner) -> std::io::Result<Self> {
            let inr = ArcMut::from_raws(p)?;
            Ok(Self { inner: inr })
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

        println!("ruis incount1={}", ruis.inner.arc_count());
        std::mem::drop(ruis1);
        println!("ruis incount2={}", ruis.inner.arc_count());
        let raw = ruis.inner.into_raw();
        println!("ruis incount1-3={}", ruis.inner.arc_count());
        let ruis2 = unsafe { Ruis::from_raw(raw).unwrap() };
        println!("ruis incount1-4={}", ruis.inner.arc_count());
        std::mem::drop(ruis2);
        println!("ruis incount1-5={}", ruis.inner.arc_count());

        let raw = ruis.inner.into_raw();
        println!("ruis incount2-3={}", ruis.inner.arc_count());
        let ruis2 = unsafe { Ruis::from_raws(raw).unwrap() };
        println!("ruis incount2-4={}", ruis.inner.arc_count());
        std::mem::drop(ruis2);
        println!("ruis incount2-5={}", ruis.inner.arc_count());

        let ruis2 = unsafe { Ruis::from_raws(raw).unwrap() };
        println!("ruis incount3-4={}", ruis.inner.arc_count());
        std::mem::drop(ruis2);
        println!("ruis incount3-5={}", ruis.inner.arc_count());

        std::mem::drop(unsafe { Ruis::from_raw(raw).unwrap() });
        println!("ruis incountEnd={}", ruis.inner.arc_count());
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
        match crate::strptime("2022-02-10T15:09:12.309627600+08:00", "%+") {
            Err(e) => println!("strptime err:{}", e),
            Ok(v) => println!("parse:{}", crate::strftime(v.clone(), "%+")),
        }
    }

    #[test]
    fn bts() {
        let bs = b"hellomgr";
        unsafe { println!("test:{}", std::str::from_utf8_unchecked(&bs[1..3])) };
        let ctx = Context::background(None);
        let mut buf = CircleBuf::new(&ctx, 1024);

        let ln = match buf.borrow_write_buf(10240) {
            Err(e) => {
                println!("borrow_write_buf err:{}", e);
                0
            }
            Ok(bts) => {
                let bs = b"mgr";
                bts[..bs.len()].copy_from_slice(bs);
                bs.len()
            }
        };
        buf.borrow_write_ok(ln);

        match buf.borrow_read_buf(10) {
            Err(e) => println!("borrow_read_buf err:{}", e),
            Ok(v) => {
                unsafe { println!("borrow_read_buf bts:{}", std::str::from_utf8_unchecked(v)) };
                buf.borrow_read_ok(v.len());
            }
        }
        match buf.get_byte(0) {
            Ok(v) => println!("bs:{}", v),
            Err(e) => println!("err:{}", e),
        }
    }

    #[test]
    fn wgs() {
        task::block_on(async move {
            let wg = crate::WaitGroup::new();
            let wgc = wg.clone();
            task::spawn(async move {
                let mut n = 0;
                while n < 30 * 100 * 2 {
                    n += 1;
                    task::sleep(Duration::from_millis(5)).await;
                }
                println!("task end1!!!!");
                std::mem::drop(wgc);
            });
            let wgc = wg.clone();
            task::spawn(async move {
                let mut n = 0;
                while n < 40 * 100 * 2 {
                    n += 1;
                    task::sleep(Duration::from_millis(5)).await;
                }
                println!("task end2!!!!");
                std::mem::drop(wgc);
            });
            println!("start waits!!!!");
            wg.waits().await;
            // task::sleep(Duration::from_secs(40)).await;
            println!("the end!!!!");
        });
    }
}
