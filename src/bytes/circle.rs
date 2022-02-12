use std::{
    io::{self, Read, Write},
    sync::Arc,
    time::Duration,
};

use async_std::task;

pub struct CircleBuf {
    ctx: crate::Context,
    data: Box<[u8]>,

    start: usize,
    end: usize,
    size: usize,
}

impl CircleBuf {
    pub fn new(ctx: &crate::Context, ln: usize) -> Self {
        Self {
            ctx: crate::Context::background(Some(ctx.clone())),
            data: vec![0u8; ln].into_boxed_slice(),

            start: 0,
            end: 0,
            size: ln,
        }
    }

    pub fn close(&self) {
        self.ctx.stop();
    }
    pub fn closed(&self) -> bool {
        self.ctx.done()
    }
    pub fn len(&self) -> usize {
        if self.start == self.end {
            0
        } else if self.start < self.end {
            self.end - self.start
        } else {
            self.size - (self.start - self.end)
        }
    }
    pub fn avail(&self) -> usize {
        self.size - self.len()
    }
    pub fn clear(&mut self) {
        self.start = 0;
        self.end = 0;
    }

    pub fn put_byte(&mut self, b: u8) -> io::Result<()> {
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        let mut pos = self.end + 1;
        if pos == self.size {
            pos = 0;
        }
        if pos == self.start {
            return Err(crate::ioerr(
                "not has available buf",
                Some(io::ErrorKind::OutOfMemory),
            ));
        }
        self.data[self.end] = b;
        self.end = pos;
        Ok(())
    }
    pub fn pop_byte(&mut self) -> io::Result<u8> {
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        if self.len() <= 0 {
            return Err(crate::ioerr(
                "not has available buf",
                Some(io::ErrorKind::InvalidData),
            ));
        }
        let rt = self.data[self.start];
        let mut pos = self.start + 1;
        if pos == self.size {
            pos = 0;
        }
        self.start = pos;
        Ok(rt)
    }
    pub async fn ayc_sleep(&self) -> io::Result<()> {
        if self.closed() {
            Err(crate::ioerr("ctx is end", None))
        } else {
            task::sleep(Duration::from_millis(1)).await;
            Ok(())
        }
    }
    pub async fn ayc_put_byte(&mut self, b: u8) -> io::Result<()> {
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        loop {
            match self.put_byte(b) {
                Err(e) => {
                    if e.kind() != io::ErrorKind::OutOfMemory {
                        return Err(e);
                    }
                }
                Ok(_) => return Ok(()),
            }
            self.ayc_sleep().await?;
        }
    }
    pub async fn ayc_pop_byte(&mut self) -> io::Result<u8> {
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        loop {
            match self.pop_byte() {
                Err(e) => {
                    if e.kind() != io::ErrorKind::InvalidData {
                        return Err(e);
                    }
                }
                Ok(v) => return Ok(v),
            }
            self.ayc_sleep().await?;
        }
    }
    pub fn get_byte(&self, i: usize) -> io::Result<u8> {
        if i >= self.len() {
            return Err(crate::ioerr("out of data buf", None));
        }
        let mut pos = self.start + i;
        if pos >= self.size {
            pos -= self.size;
        }
        Ok(self.data[pos])
    }
    pub fn borrow_read_buf(&self, ln: usize) -> io::Result<&[u8]> {
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        if self.len() <= 0 {
            return Err(crate::ioerr(
                "not has available buf",
                Some(io::ErrorKind::InvalidData),
            ));
        }
        let mut lns = self.start + ln;
        if self.start < self.end {
            if lns > self.end {
                lns = self.end;
            }
        } else {
            if lns > self.size {
                lns = self.size
            }
        }
        Ok(&self.data[self.start..lns])
    }
    pub fn borrow_read_ok(&mut self, ln: usize) -> io::Result<()> {
        if ln<=0{
            return Ok(());
        }
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        if ln > self.len() {
            return Err(crate::ioerr("out of data buf", None));
        }
        let mut pos = self.start + ln;
        if pos > self.size {
            return Err(crate::ioerr("out of buf,please check", None));
        }
        if pos == self.size {
            pos = 0;
        }
        self.start = pos;
        Ok(())
    }
    pub fn borrow_write_buf(&mut self, ln: usize) -> io::Result<&mut [u8]> {
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        if self.avail() <= 0 {
            return Err(crate::ioerr(
                "not has available buf",
                Some(io::ErrorKind::InvalidData),
            ));
        }
        let mut lns = self.end + ln;
        if self.end < self.start {
            if lns > self.start {
                lns = self.start;
            }
        } else {
            if lns > self.size {
                lns = self.size
            }
        }
        Ok(&mut self.data[self.end..lns])
    }
    pub fn borrow_write_ok(&mut self, ln: usize) -> io::Result<()> {
        if ln<=0{
            return Ok(());
        }
        if self.closed() {
            return Err(crate::ioerr("ctx is end", None));
        }
        if ln > self.avail() {
            return Err(crate::ioerr("out of data buf", None));
        }
        let mut pos = self.end + ln;
        if pos > self.size {
            return Err(crate::ioerr("out of buf,please check", None));
        }
        if pos == self.size {
            pos = 0;
        }
        self.end = pos;
        Ok(())
    }
}

impl async_std::io::Read for CircleBuf {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> task::Poll<io::Result<usize>> {
        if self.closed() {
            return task::Poll::Ready(Err(crate::ioerr("ctx is end", None)));
        }
        if self.len() <= 0 {
            return task::Poll::Pending;
        }
        let bufs = self.borrow_read_buf(buf.len())?;
        let ln = bufs.len();
        buf[..ln].copy_from_slice(bufs);
        if let Err(e) = self.borrow_read_ok(ln) {
            return task::Poll::Ready(Err(e));
        }
        task::Poll::Ready(Ok(ln))
    }
}
impl async_std::io::Write for CircleBuf {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> task::Poll<io::Result<usize>> {
        if self.closed() {
            return task::Poll::Ready(Err(crate::ioerr("ctx is end", None)));
        }
        if self.avail() <= 0 {
            return task::Poll::Pending;
        }
        let bufs = self.borrow_write_buf(buf.len())?;
        let ln = bufs.len();
        bufs.copy_from_slice(&buf[..bufs.len()]);
        std::mem::drop(bufs);
        self.borrow_write_ok(ln)?;
        task::Poll::Ready(Ok(ln))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<io::Result<()>> {
        task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<io::Result<()>> {
        self.close();
        task::Poll::Ready(Ok(()))
    }
}

impl Read for CircleBuf {
    /* fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut ln = 0;
        for b in buf {
            match self.pop_byte() {
                Err(e) => {
                    if e.kind() != io::ErrorKind::InvalidData {
                        return Err(e);
                    } else
                    /*  if ln>0 */
                    {
                        break;
                    }
                }
                Ok(v) => {
                    *b = v;
                    ln += 1
                }
            }
        }
        Ok(ln)
    } */
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bufs = self.borrow_read_buf(buf.len())?;
        let ln = bufs.len();
        buf[..ln].copy_from_slice(bufs);
        self.borrow_read_ok(ln)?;
        Ok(ln)
    }
}
impl Write for CircleBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bufs = self.borrow_write_buf(buf.len())?;
        let ln = bufs.len();
        bufs.copy_from_slice(&buf[..bufs.len()]);
        std::mem::drop(bufs);
        self.borrow_write_ok(ln)?;
        Ok(ln)
    }

    fn flush(&mut self) -> io::Result<()> {
        // self.clear();
        Ok(())
    }
}
