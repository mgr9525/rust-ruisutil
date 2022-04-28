use std::{
    collections::{linked_list, LinkedList},
    io::{self, Read, Write},
    ops::Deref,
    sync::Arc,
};

use crate::ioerr;

#[derive(Clone)]
pub struct ByteBox {
    start: usize,
    end: usize,
    data: Arc<Box<[u8]>>,
}
impl Deref for ByteBox {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.data[self.start..self.end]
    }
}
impl ByteBox {
    pub fn new(dt: Arc<Box<[u8]>>, start: usize, end: usize) -> Self {
        Self {
            start: start,
            end: if end > 0 { end } else { dt.len() },
            data: dt,
        }
    }
    pub fn cut(&mut self, pos: usize) -> io::Result<Self> {
        let posd = pos + self.start;
        if posd < self.start || posd >= self.end {
            Err(ioerr("pos err", None))
        } else {
            let rt = Self {
                start: posd,
                end: self.end,
                data: self.data.clone(),
            };
            self.end = posd;
            Ok(rt)
        }
    }
    /* pub fn cut_front(&mut self, pos: usize) -> io::Result<Self> {
        let posd = pos + self.start;
        if posd < self.start || posd >= self.end {
            Err(ioerr("pos err", None))
        } else {
            let rt = Self {
                start: self.start,
                end: posd,
                data: self.data.clone(),
            };
            self.start = posd;
            Ok(rt)
        }
    } */
    /* pub fn cut_front(&mut self, pos: usize) -> io::Result<Self> {
        if pos <= self.start || pos >= self.end {
            Err(ioerr("pos err", None))
        } else {
            let c = Self {
                start: self.start,
                end: pos,
                data: self.data.clone(),
            };
            self.start = pos;
            Ok(c)
        }
    } */

    pub fn clones(&self, start: usize, end: usize) -> io::Result<Self> {
        if start < self.start || end > self.end {
            Err(ioerr("len err", None))
        } else {
            Ok(Self {
                start: start,
                end: end,
                data: self.data.clone(),
            })
        }
    }
}
impl From<Vec<u8>> for ByteBox {
    fn from(v: Vec<u8>) -> Self {
        let ln = v.len();
        Self::new(Arc::new(v.into_boxed_slice()), 0, ln)
    }
}
impl From<Box<[u8]>> for ByteBox {
    fn from(v: Box<[u8]>) -> Self {
        let ln = v.len();
        Self::new(Arc::new(v), 0, ln)
    }
}
impl From<Arc<Box<[u8]>>> for ByteBox {
    fn from(v: Arc<Box<[u8]>>) -> Self {
        let ln = v.len();
        Self::new(v, 0, ln)
    }
}

pub struct ByteBoxBuf {
    count: usize,
    list: LinkedList<ByteBox>,
}
impl ByteBoxBuf {
    pub fn new() -> Self {
        Self {
            count: 0,
            list: LinkedList::new(),
        }
    }
    fn push_front(&mut self, data: ByteBox) {
        if data.len() > 0 {
            self.count += data.len();
            self.list.push_front(data);
        }
    }
    pub fn push<T: Into<ByteBox>>(&mut self, data: T) {
        let dt = data.into();
        if dt.len() > 0 {
            self.count += dt.len();
            self.list.push_back(dt);
        }
    }
    pub fn push_all(&mut self, data: &Self) {
        for v in data.iter() {
            self.push(v.clone());
        }
    }
    pub fn pushs(&mut self, dt: Arc<Box<[u8]>>, start: usize, end: usize) {
        let data = ByteBox::new(dt, start, end);
        self.push(data);
    }
    pub fn push_start(&mut self, dt: Arc<Box<[u8]>>, start: usize) {
        let ln = dt.len();
        if ln > 0 {
            self.pushs(dt, start, ln);
        }
    }
    pub fn push_len(&mut self, dt: Arc<Box<[u8]>>, len: usize) -> usize {
        let mut ln = dt.len();
        if len < ln {
            ln = len;
        }
        if ln > 0 {
            self.pushs(dt, 0, ln);
        }

        ln
    }
    pub fn pull(&mut self) -> Option<ByteBox> {
        match self.list.pop_front() {
            None => None,
            Some(v) => {
                self.count -= v.len();
                Some(v)
            }
        }
    }
    pub fn pull_all(&mut self) -> Self {
        let mut rts = Self::new();
        while let Some(v) = self.pull() {
            rts.push(v);
        }
        rts
    }
    pub fn clear(&mut self) {
        self.list.clear();
    }
    pub fn iter(&self) -> linked_list::Iter<ByteBox> {
        self.list.iter()
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn lens(&self) -> usize {
        let mut rts = 0;
        let mut itr = self.list.iter();
        while let Some(v) = itr.next() {
            rts += v.len();
        }
        rts
    }
    pub fn gets(&self, start: usize, len: usize) -> io::Result<(Box<[u8]>, usize)> {
        if len <= 0 {
            return Err(ioerr("len err", None));
        }
        if start >= self.count || start + len > self.count {
            return Err(ioerr("pos out limit", None));
        }
        let mut rtbts: Vec<u8> = Vec::new();
        let mut start_real = start;
        let mut len_real = len;
        let mut itr = self.list.iter();
        'ends: while let Some(v) = itr.next() {
            let ln = v.len();
            if start_real < ln {
                for b in &v[start_real..] {
                    rtbts.push(*b);
                    len_real -= 1;
                    // println!("test len_real:{}/{}", len_real, len);
                    if len_real <= 0 {
                        break 'ends;
                    }
                }
                start_real = 0;
            } else {
                start_real -= ln;
            }
        }

        if len != rtbts.len() {
            return Err(ioerr(
                format!(
                    "get len err:{}/{},list:{}",
                    rtbts.len(),
                    len,
                    self.list.len()
                ),
                None,
            ));
        }

        Ok((rtbts.into_boxed_slice(), start + len))
    }
    pub fn cut_front(&mut self, pos: usize) -> io::Result<Self> {
        if pos <= 0 {
            return Err(ioerr("pos err", None));
        }
        if pos >= self.count {
            return Err(ioerr("pos out limit", None));
        }
        let mut frt = Self::new();
        let mut pos_real = pos;
        while let Some(mut v) = self.pull() {
            let ln = v.len();
            if pos_real < ln {
                let rgt = v.cut(pos_real)?;
                frt.push(v);
                self.push_front(rgt);
                break;
            } else {
                pos_real -= ln;
                frt.push(v);
            }
        }

        Ok(frt)
    }
    pub fn to_bytes(&self) -> Box<[u8]> {
        let mut rtbts: Vec<u8> = Vec::with_capacity(self.count);
        let mut itr = self.list.iter();
        while let Some(v) = itr.next() {
            // rtbts.copy_from_slice(src)
            for b in &v[..] {
                rtbts.push(*b);
            }
        }
        rtbts.into_boxed_slice()
    }
}

impl Read for ByteBoxBuf {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(mut it) = self.pull() {
            /* println!(
                "test ByteBoxBuf read:{}/{}/{}",
                buf.len(),
                it.len(),
                self.len()
            ); */
            if buf.len() == it.len() {
                buf.copy_from_slice(&it[..]);
                return Ok(it.len());
            } else if buf.len() > it.len() {
                let bufs = &mut buf[..it.len()];
                bufs.copy_from_slice(&it[..it.len()]);
                return Ok(it.len());
            } else if buf.len() < it.len() {
                if let Ok(rgt) = it.cut(buf.len()) {
                    self.push_front(rgt);
                }
                buf.copy_from_slice(&it[..]);
                return Ok(it.len());
            }
        };
        Ok(0)
    }
}
impl Write for ByteBoxBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut bts = vec![0u8; buf.len()].into_boxed_slice();
        bts.copy_from_slice(buf);
        self.push_start(Arc::new(bts), 0);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // self.clear();
        Ok(())
    }
}
