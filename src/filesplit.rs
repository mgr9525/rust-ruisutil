use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, MutexGuard,
    },
    time::Duration,
};

#[derive(Clone)]
pub struct FileSpliter {
    inner: crate::ArcMut<Inner>,
}
pub struct Config {
    pub maxbuf: usize,
    pub flpath: String,
    pub flsize: u32,  // file size limit(KB)
    pub flcount: u32, // file count
}
struct Inner {
    ctx: crate::Context,
    cfg: Config,
    buf: Mutex<crate::ListDequeMax<Box<[u8]>>>,
    wkr: crate::sync::Waker,
    flfd: Mutex<Option<File>>,
    flln: AtomicUsize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            maxbuf: 0,
            flpath: String::new(),
            flsize: 0,
            flcount: 0,
        }
    }
}
impl FileSpliter {
    pub fn new(ctx: &crate::Context, mut cg: Config) -> Self {
        if cg.maxbuf <= 0 {
            cg.maxbuf = 20;
        }
        if cg.flsize <= 0 {
            cg.flsize = 1024;
        }
        if cg.flcount <= 0 {
            cg.flcount = 1;
        }
        let maxbuf = cg.maxbuf;
        let ctxs = crate::Context::background(Some(ctx.clone()));
        Self {
            inner: crate::ArcMut::new(Inner {
                ctx: ctxs.clone(),
                cfg: cg,
                buf: Mutex::new(crate::ListDequeMax::new(maxbuf)),
                wkr: crate::sync::Waker::new(&ctxs),
                flfd: Mutex::new(None),
                flln: AtomicUsize::new(0),
            }),
        }
    }

    pub fn stop(&self) {
        self.inner.ctx.stop();
        self.inner.wkr.close();
    }

    pub fn run(&self) -> io::Result<()> {
        {
            if self.inner.cfg.flpath.is_empty() {
                return Err(crate::ioerr("file path is empty", None));
            }
            let pth = Path::new(self.inner.cfg.flpath.as_str());
            if pth.exists() && pth.is_dir() {
                return Err(crate::ioerr("file path is err", None));
            }
            if let Some(e) = pth.parent() {
                if e.exists() && !e.is_dir() {
                    return Err(crate::ioerr("file path is err", None));
                }
                std::fs::create_dir(e);
            }
        }
        while !self.inner.ctx.done() {
            let out = {
                if let Ok(mut lkv) = self.inner.buf.lock() {
                    lkv.pop()
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };
            if let Some(v) = out {
                self.appends(&v[..]);
            } else {
                self.inner.wkr.wait_timeout(Duration::from_millis(100));
            }
        }
        if let Ok(mut lkv) = self.inner.flfd.lock() {
            *lkv = None;
            self.inner.flln.store(0, Ordering::SeqCst);
        }
        Ok(())
    }
    fn appends(&self, bts: &[u8]) {
        let ctxs =
            crate::Context::with_timeout(Some(self.inner.ctx.clone()), Duration::from_secs(5));
        let mut flfd = None;
        while !ctxs.done() {
            if let Ok(v) = self.inner.flfd.lock() {
                flfd = Some(v);
                break;
            }
        }
        if let Some(mut lkv) = flfd {
            while !ctxs.done() {
                if let Err(e) = self.checkfl(&mut lkv) {
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    println!("filespliter.appends.checkfl err:{}", e);
                    return;
                } else {
                    break;
                }
            }
            if let Some(fle) = &mut *lkv {
                crate::write_all(&self.inner.ctx, fle, bts);
                self.inner.flln.fetch_add(bts.len(), Ordering::SeqCst);
            }
        }
    }
    fn checkfl(&self, lkv: &mut MutexGuard<Option<File>>) -> io::Result<()> {
        match &mut **lkv {
            Some(v) => {
                let max = self.inner.cfg.flsize as usize * 1024;
                if self.inner.flln.load(Ordering::SeqCst) > max {
                    // self.copyfl(v);
                    // v.seek(std::io::SeekFrom::Start(0))?;
                    std::mem::drop(v);
                    **lkv = None;
                    std::thread::sleep(Duration::from_millis(10));
                    self.movefls();
                    return Err(crate::ioerr(
                        "need open",
                        Some(std::io::ErrorKind::Interrupted),
                    ));
                    /* let v = File::create(self.inner.cfg.flpath.as_str())?;
                    **lkv = Some(v);
                    self.inner.flln.store(0, Ordering::SeqCst); */
                }
            }
            None => {
                let pth = self.inner.cfg.flpath.as_str();
                let sz = match std::fs::metadata(pth) {
                    Ok(v) => v.len(),
                    Err(_e) => 0,
                };
                let v = std::fs::OpenOptions::new()
                    .read(true)
                    .append(true)
                    .create(true)
                    .open(pth)?;
                **lkv = Some(v);
                self.inner.flln.store(sz as usize, Ordering::SeqCst);
            }
        }
        Ok(())
    }
    fn copyfl(&self, fle: &mut File) {
        if self.inner.cfg.flcount <= 1 {
            return;
        }
        if let Err(e) = fle.seek(std::io::SeekFrom::Start(0)) {
            return;
        }
        self.movefls();
        let mut flne = match File::create(format!("{}.{}", &self.inner.cfg.flpath, 1)) {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut buf = vec![0u8; 1024].into_boxed_slice();
        while !self.inner.ctx.done() {
            match fle.read(&mut buf) {
                Ok(n) => {
                    if n <= 0 {
                        break;
                    }
                    crate::utils::write_all(&self.inner.ctx, &mut flne, &buf[..n]);
                }
                Err(_) => break,
            }
        }
    }

    fn movefls(&self) {
        if self.inner.cfg.flcount <= 1 {
            std::fs::remove_file(&self.inner.cfg.flpath);
            return;
        }
        let mut i = self.inner.cfg.flcount - 1;
        std::fs::remove_file(format!("{}.{}", &self.inner.cfg.flpath, i));
        while i > 1 {
            std::fs::rename(
                format!("{}.{}", &self.inner.cfg.flpath, i - 1),
                format!("{}.{}", &self.inner.cfg.flpath, i),
            );
            i -= 1;
        }
        std::fs::rename(
            &self.inner.cfg.flpath,
            format!("{}.{}", &self.inner.cfg.flpath, 1),
        );
    }

    pub fn push(&self, bts: &[u8]) {
        if let Ok(mut lkv) = self.inner.buf.lock() {
            lkv.push(bts.to_vec().into_boxed_slice());
            self.inner.wkr.notify_all();
        }
    }
    pub fn pushbox(&self, bts: Box<[u8]>) {
        if let Ok(mut lkv) = self.inner.buf.lock() {
            lkv.push(bts);
            self.inner.wkr.notify_all();
        }
    }
    pub fn pushs(&self, conts: &str) {
        if let Ok(mut lkv) = self.inner.buf.lock() {
            lkv.push(conts.as_bytes().to_vec().into_boxed_slice());
            self.inner.wkr.notify_all();
        }
    }
    pub fn flush(&self) {
        if let Ok(mut lkv) = self.inner.flfd.lock() {
            if let Some(v) = &mut *lkv {
                v.flush();
            }
        }
    }
}
