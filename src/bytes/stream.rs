use async_std::prelude::*;
use std::io::{self, Write};

use crate::Context;

use super::ByteBoxBuf;

pub fn tcp_write(
    ctx: &Context,
    stream: &mut std::net::TcpStream,
    buf: &ByteBoxBuf,
) -> io::Result<usize> {
    if buf.len() <= 0 {
        return Ok(0);
    }
    let mut wnz = 0usize;
    let mut itr = buf.iter();
    while let Some(bts) = itr.next() {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        let mut wn = 0usize;
        while wn < bts.len() {
            if ctx.done() {
                return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
            }
            match stream.write(&bts[wn..]) {
                Err(e) => return Err(e),
                Ok(n) => {
                    if n > 0 {
                        wn += n;
                        wnz += n;
                    } else {
                        // let bts=&data[..];
                        // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                        return Err(io::Error::new(io::ErrorKind::Other, "write err!"));
                    }
                }
            }
        }
    }
    Ok(wnz)
}

pub async fn tcp_write_async(
    ctx: &Context,
    stream: &mut async_std::net::TcpStream,
    buf: &ByteBoxBuf,
) -> io::Result<usize> {
    if buf.len() <= 0 {
        return Ok(0);
    }
    let mut wnz = 0usize;
    let mut itr = buf.iter();
    while let Some(bts) = itr.next() {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
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
                        wnz += n;
                    } else {
                        // let bts=&data[..];
                        // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                        return Err(io::Error::new(io::ErrorKind::Other, "write err!"));
                    }
                }
            }
        }
    }
    Ok(wnz)
}
