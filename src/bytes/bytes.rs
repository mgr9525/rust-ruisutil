use std::{
    collections::{linked_list, LinkedList},
    io::{self, Read, Write},
    ops::Deref,
    sync::Arc,
};

use bytes::BufMut;

use crate::ioerr;

/* #[derive(Clone)]
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
    pub fn news(dt: Box<[u8]>, start: usize, end: usize) -> Self {
        Self {
            start: start,
            end: end,
            data: Arc::new(dt),
        }
    }
    pub fn newlen(dt: Box<[u8]>, end: usize) -> Self {
        Self::news(dt, 0, end)
    }
    pub fn cut(&mut self, pos: usize) -> io::Result<Self> {
        let posd = pos + self.start;
        if posd < self.start || posd > self.end {
            Err(ioerr(
                format!(
                    "ByteBox.cut pos err:posd={},s={},e={}",
                    posd, self.start, self.end
                ),
                None,
            ))
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
    pub fn cuts(&mut self, pos: usize) -> io::Result<Self> {
        let posd = pos + self.start;
        if posd < self.start || posd > self.end {
            Err(ioerr(
                format!(
                    "ByteBox.cuts pos err:posd={},s={},e={}",
                    posd, self.start, self.end
                ),
                None,
            ))
        /* }else if posd == self.end {
        let rt = Self {
            start: self.start,
            end: posd,
            data: self.data.clone(),
        }; */
        } else {
            let rt = Self {
                start: self.start,
                end: posd,
                data: self.data.clone(),
            };
            self.start = posd;
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

    /* pub fn bytes(&mut self) -> Box<[u8]> {
        let tmp = Vec::new().into_boxed_slice();
        let bts = std::mem::replace(&mut self.data, Arc::new(tmp));
        let t=Arc::downgrade(&bts);
        *t
    } */
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
impl From<&[u8]> for ByteBox {
    fn from(v: &[u8]) -> Self {
        Self::from(v.to_vec())
    }
} */

#[derive(Clone)]
pub struct ByteBoxBuf {
    count: usize,
    list: LinkedList<bytes::Bytes>,
}
impl ByteBoxBuf {
    pub fn new() -> Self {
        Self {
            count: 0,
            list: LinkedList::new(),
        }
    }
    pub fn push_front<T: Into<bytes::Bytes>>(&mut self, data: T) {
        let dt = data.into();
        if dt.len() > 0 {
            self.count += dt.len();
            self.list.push_front(dt);
        }
    }
    pub fn push<T: Into<bytes::Bytes>>(&mut self, data: T) {
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
    pub fn pushs(&mut self, mut dt: Vec<u8>, n: usize) {
        // let data = bytes::Bytes::new(dt, start, end);
        dt.truncate(n);
        self.push(dt);
    }
    /* pub fn push_start(&mut self, dt: Arc<Box<[u8]>>, start: usize) {
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
    } */
    pub fn pull(&mut self) -> Option<bytes::Bytes> {
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
        self.count = 0;
    }
    pub fn iter(&self) -> linked_list::Iter<bytes::Bytes> {
        self.list.iter()
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn frtlen(&self) -> usize {
        if let Some(v) = self.list.front() {
            v.len()
        } else {
            0
        }
    }
    pub fn lens(&self) -> usize {
        let mut rts = 0;
        let itr = self.list.iter();
        for v in itr {
            rts += v.len();
        }
        rts
    }
    pub fn get_byte(&self, idx: usize) -> io::Result<u8> {
        if idx >= self.len() {
            return Err(ioerr("idx err:more count", None));
        }
        let mut lns = 0usize;
        let itr = self.list.iter();
        for v in itr {
            let idxs = idx - lns;
            if idxs < v.len() {
                return Ok(v[idxs]);
            }
            lns += v.len();
        }
        Err(ioerr("not found index byte", None))
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
        let itr = self.list.iter();
        'ends: for v in itr {
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
        if pos > self.count {
            return Err(ioerr("cut_front pos out limit", None));
        }
        let mut frt = Self::new();
        if pos <= 0 {
            return Ok(frt);
        }
        let mut pos_real = pos;
        while let Some(mut v) = self.pull() {
            let dt = v.split_to(pos_real);
            if v.len() > 0 {
                self.push_front(v);
            }
            pos_real -= dt.len();
            frt.push(dt);
            if pos_real <= 0 {
                break;
            }
            /* let ln = v.len();
            if pos_real < ln {
                let rgt = v.cuts(pos_real)?;
                frt.push(rgt);
                self.push_front(v);
                break;
            } else {
                pos_real -= ln;
                frt.push(v);
                if pos_real <= 0 {
                    break;
                }
            } */
        }

        Ok(frt)
    }
    /* pub fn to_bytes(&self) -> Box<[u8]> {
        let mut rtbts: Vec<u8> = Vec::with_capacity(self.count);
        let mut itr = self.list.iter();
        while let Some(v) = itr.next() {
            // rtbts.copy_from_slice(src)
            for b in &v[..] {
                rtbts.push(*b);
            }
        }
        rtbts.into_boxed_slice()
    } */
    pub fn to_bytes(&self) -> bytes::Bytes {
        let mut buf = bytes::BytesMut::with_capacity(self.count);
        // let mut pos = 0usize;
        // let mut rtbts = vec![0u8; self.count].into_boxed_slice();
        let itr = self.list.iter();
        // while let Some(v) = itr.next() {
        for v in itr {
            buf.put(v.clone());
            // let end = pos + v.len();
            // (&mut rtbts[pos..end]).copy_from_slice(&v[..]);
            // pos = end;
        }
        buf.freeze()
        // rtbts
    }
    /* pub fn to_byte_box(&self) -> bytes::Bytes {
        if self.list.len() == 1 {
            if let Some(bts) = self.list.front() {
                return bts.clone();
            }
        }
        bytes::Bytes::from(self.to_bytes())
    } */
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
            let dt = it.split_to(buf.len());
            if it.len() > 0 {
                self.push_front(it);
            }
            buf.copy_from_slice(&dt[..]);
        };
        Ok(0)
    }
}
impl Write for ByteBoxBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bts = bytes::Bytes::copy_from_slice(buf);
        self.push(bts);
        /* let mut bts = vec![0u8; buf.len()].into_boxed_slice();
        bts.copy_from_slice(buf);
        self.push(Arc::new(bts)); */
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // self.clear();
        Ok(())
    }
}
