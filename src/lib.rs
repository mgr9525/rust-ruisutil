use std::{
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc,
    },
    time::{self, Duration},
};

pub use contianer::ArcMut;
pub use list::ListDequeMax;
pub use timer::Timer;
pub use utils::*;

pub mod bytes;
pub mod conf;
mod contianer;
#[cfg(feature = "filesplit")]
pub mod filesplit;
mod list;
#[cfg(feature = "logs")]
pub mod log;
pub mod message;
pub mod sync;
mod timer;
mod utils;

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
    wkr1: sync::Waker,
    wkr2: sync::WakerFut,
}
impl WaitGroup {
    pub fn new() -> Self {
        let ctx = Context::background(None);
        Self {
            inner: Arc::new(WgInner {
                count: AtomicI32::new(0),
                wkr1: sync::Waker::new(&ctx),
                wkr2: sync::WakerFut::new(&ctx),
            }),
        }
    }
    pub fn wait(&self, ctxs: Option<Context>) {
        loop {
            if let Some(v) = &ctxs {
                if v.done() {
                    break;
                }
            }
            self.inner.wkr1.wait_timeout(Duration::from_millis(100));
            if self.done() {
                break;
            }
        }
    }
    #[cfg(feature = "asyncs")]
    pub async fn waits(&self, ctxs: Option<Context>) {
        loop {
            if let Some(v) = &ctxs {
                if v.done() {
                    break;
                }
            }
            async_std::io::timeout(Duration::from_millis(100), self.inner.wkr2.clone()).await;
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
        self.inner.wkr1.notify_all();
        self.inner.wkr2.notify_all();
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
    use std::time::{Duration, SystemTime};

    use crate::{bytes::CircleBuf, conf::KVConfig, ArcMut, Context};

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
        println!("randtms:{}", crate::randtms());
        println!("randoms:{}", crate::randoms());
        println!("randtms_ang:{}", crate::randtms_ang(10, 60));
        println!("random:{}", crate::random(32));
    }
    #[test]
    fn randtms() {
        let (tms, _) = crate::times();
        let tmsi = tms.as_nanos();
        // let tmsibts=tmsi.to_le_bytes();
        let tmsix = (tmsi & 0xffffffff) as u32;
        println!("tmsi:{}, tmsix:{}", tmsi, tmsix);
    }

    #[test]
    fn md5s() {
        let md5s = "48c4a8a547cadf2e245e867bcb4c27a6";
        let tms = crate::strftime(SystemTime::now(), "%+");
        let rands = crate::randoms();
        let srcs = format!("{}{}{}{}", &md5s, &tms, &rands, "asdfasdf");
        println!("srcs:{}", &srcs);
        let signs = crate::md5str(srcs);
        println!("md5s:{}", &signs);
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

        let tm1 = crate::strftime_off(now.clone(), "%+", 10);
        let tm2 = crate::strftime_off(now.clone(), "%Y-%m-%d %H:%M:%S", 10);
        println!("tm1={}", &tm1);
        println!("tm2={}", &tm2);
        match crate::strptime_off(&tm1, "%+", 10) {
            Ok(v) => println!("strptime tm1[12] ok:{}", crate::strftime(v.clone(), "%+")),
            Err(e) => println!("strptime tm1[12] err:{}", e),
        }
        match crate::strptime_off(
            format!("{}.mp4", &tm2).as_str(),
            "%Y-%m-%d %H:%M:%S.mp4",
            10,
        ) {
            Ok(v) => println!(
                "strptime tm2[12] ok:{},{}",
                crate::strftime(v.clone(), "%+"),
                crate::strftime_off(v.clone(), "%Y-%m-%d %H:%M:%S", 10)
            ),
            Err(e) => println!("strptime tm2[12] err:{}", e),
        }
        println!("end!!!!!!");
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

    #[cfg(feature = "asyncs")]
    #[test]
    fn wgs() {
        let wgt = crate::WaitGroup::new();
        let wgtcg = wgt.clone();
        async_std::task::block_on(async move {
            let wg = crate::WaitGroup::new();
            let wgc = wg.clone();
            let wgtc = wgtcg.clone();
            async_std::task::spawn(async move {
                let mut n = 0;
                while n < 30 * 100 * 2 {
                    n += 1;
                    async_std::task::sleep(Duration::from_millis(5)).await;
                }
                println!("task end1!!!!");
                std::mem::drop(wgc);
                std::mem::drop(wgtc);
            });
            let wgc = wg.clone();
            let wgtc = wgtcg.clone();
            async_std::task::spawn(async move {
                let mut n = 0;
                while n < 40 * 100 * 2 {
                    n += 1;
                    async_std::task::sleep(Duration::from_millis(5)).await;
                }
                println!("task end2!!!!");
                std::mem::drop(wgc);
                std::mem::drop(wgtc);
            });
            let wgtc = wgtcg.clone();
            std::thread::spawn(move || {
                let mut n = 0;
                while n < 50 * 100 * 2 {
                    n += 1;
                    std::thread::sleep(Duration::from_millis(5));
                }
                println!("task end3!!!!");
                std::mem::drop(wgtc);
            });
            println!("start waits!!!!");
            wg.waits(None).await;
            println!("the end1!!!!");
        });
        wgt.wait(None);
        println!("the end2!!!!");
    }

    #[test]
    fn kvcfg() {
        let cfgs = KVConfig::from_bytes(
            b"abc=123
        hahah=
        123124124
        ruis= shuai",
        );
        for (k, v) in cfgs.iter() {
            println!("cfg: {} = {}", k, v);
        }

        println!("-----------------parse end");
        println!("tos:\n{}", cfgs.to_string());
        println!("-----------------tos end");
    }

    #[test]
    fn hex() {
        crate::print_hex(&vec![0xaa, 0xb3, 0x0a, 0x0c, 0x00]);
    }

    #[test]
    fn filesplits() {
        let cfg = crate::filesplit::Config {
            flpath: "/mnt/e/wslData/programs/rust/rust-ruisutil/test/ruisutil.log".to_string(),
            flsize: 1,
            flcount: 5,
            ..Default::default()
        };
        println!("------00000000:pth={}", &cfg.flpath);
        let ctx = Context::background(None);
        let flspt = crate::filesplit::FileSpliter::new(&ctx, cfg);
        let flsptc = flspt.clone();
        std::thread::spawn(move || {
            if let Err(e) = flsptc.run() {
                println!("flsptc.run err:{}", e)
            }
        });
        for i in 0..1000 {
            flspt.pushs(&format!("hello world:{}\n", i));
            std::thread::sleep(Duration::from_millis(20));
            flspt.pushs("11111\n");
            std::thread::sleep(Duration::from_millis(20));
            flspt.pushs("22222\n");
            std::thread::sleep(Duration::from_millis(20));
            flspt.pushs("3333\n");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn logs() {
        let cfg = crate::filesplit::Config {
            flpath: "/mnt/e/wslData/programs/rust/rust-ruisutil/test/ruisutils.log".to_string(),
            flsize: 1,
            flcount: 5,
            ..Default::default()
        };
        println!("------00000000:pth={}", &cfg.flpath);
        let ctx = Context::background(None);
        let mut lg = crate::log::Logger::new(&ctx, cfg);
        lg.level(log::Level::Debug).timezone(10).show_file_info().show_module();
        if let Err(e) = lg.start() {
            println!("log start err:{}", e);
            return;
        }
        println!("------1111111");
        for i in 0..1000 {
            log::info!("hello world:{}", i);
            std::thread::sleep(Duration::from_millis(20));
            log::debug!("11111");
            std::thread::sleep(Duration::from_millis(20));
            log::error!("22222");
            std::thread::sleep(Duration::from_millis(20));
            log::warn!("3333");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
