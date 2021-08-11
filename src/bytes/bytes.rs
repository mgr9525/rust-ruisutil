use std::{collections::LinkedList, io::Read, ops::Deref, sync::Arc};

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

pub struct ByteBoxBuf {
    list: LinkedList<ByteBox>,
}
impl ByteBoxBuf {
    pub fn new() -> Self {
        Self {
            list: LinkedList::new(),
        }
    }

    pub fn push(&mut self, dt: Arc<Box<[u8]>>, start: usize, end: usize) {
        let data = ByteBox {
            start: start,
            end: end,
            data: dt,
        };
        self.list.push_back(data);
    }
    pub fn push_start(&mut self, dt: Arc<Box<[u8]>>, start: usize) {
        let ln = dt.len();
        self.push(dt, start, ln - 1);
    }
    pub fn pushs(&mut self, dt: Arc<Box<[u8]>>) {
        self.push_start(dt, 0);
    }
    pub fn pull(&mut self) -> Option<ByteBox> {
        self.list.pop_front()
    }
}

/* impl Read for ByteBoxBuf {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.list.front() {
            None => Ok(0),
            Some(it) => {
                let rn = 0usize;

                Ok(rn)
            }
        }
    }
} */
