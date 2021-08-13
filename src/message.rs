use std::{io, mem, sync::Arc};

use super::bytes::{self, ByteBoxBuf};

//----------------------------------bean
#[repr(C, packed)]
struct MsgInfo {
    pub version: u16,
    pub control: i32,
    pub lenCmd: u16,
    pub lenHead: u32,
    pub lenBody: u32,
}
impl MsgInfo {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            lenCmd: 0,
            lenHead: 0,
            lenBody: 0,
        }
    }
}

pub const MaxOther: u64 = 1024 * 1024 * 20; //20M
pub const MaxHeads: u64 = 1024 * 1024 * 100; //100M
pub const MaxBodys: u64 = 1024 * 1024 * 1024; //1G

pub struct Message {
    pub version: u16,
    pub control: i32,
    pub cmds: String,
    pub heads: Option<Box<[u8]>>,
    pub bodys: Option<Box<[u8]>>,
}
impl Message {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            cmds: String::new(),
            heads: None,
            bodys: None,
        }
    }
    pub fn own_bodys(&mut self) -> Option<Box<[u8]>> {
        std::mem::replace(&mut self.bodys, None)
    }
}

pub fn parse_msg(ctxs: &super::Context, conn: &mut std::net::TcpStream) -> io::Result<Message> {
    let bts = super::tcp_read(ctxs, conn, 1)?;
    if bts.len() < 1 && bts[0] != 0x8du8 {
        return Err(super::ioerr(format!("first byte err:{:?}", &bts[..]), None));
    }
    let bts = super::tcp_read(ctxs, conn, 1)?;
    if bts.len() < 1 && bts[0] != 0x8fu8 {
        return Err(super::ioerr(format!("first byte err:{:?}", &bts[..]), None));
    }

    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let bts = super::tcp_read(ctxs, conn, infoln)?;
    super::byte2struct(&mut info, &bts[..])?;
    if (info.lenHead) as u64 > MaxHeads {
        return Err(super::ioerr("bytes2 out limit!!", None));
    }
    if (info.lenBody) as u64 > MaxBodys {
        return Err(super::ioerr("bytes3 out limit!!", None));
    }

    let mut rt = Message::new();
    rt.version = info.version;
    rt.control = info.control;
    let lnsz = info.lenCmd as usize;
    if lnsz > 0 {
        let bts = super::tcp_read(&ctxs, conn, lnsz)?;
        rt.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(super::ioerr("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.lenHead as usize;
    if lnsz > 0 {
        let bts = super::tcp_read(&ctxs, conn, lnsz as usize)?;
        rt.heads = Some(bts);
    }
    let lnsz = info.lenBody as usize;
    if lnsz > 0 {
        let bts = super::tcp_read(&ctxs, conn, lnsz as usize)?;
        rt.bodys = Some(bts);
    }
    let bts = super::tcp_read(ctxs, conn, 2)?;
    if bts.len() < 2 && bts[0] != 0x8eu8 && bts[1] != 0x8fu8 {
        return Err(super::ioerr(format!("end byte err:{:?}", &bts[..]), None));
    }

    Ok(rt)
}

pub fn send_msg(
    ctxs: &super::Context,
    conn: &mut std::net::TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: Option<Arc<Box<[u8]>>>,
    bds: Option<Arc<Box<[u8]>>>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.lenCmd = v.len() as u16;
    }
    if let Some(v) = &hds {
        info.lenHead = v.len() as u32;
    }
    if let Some(v) = &bds {
        info.lenBody = v.len() as u32;
    }
    super::tcp_write(ctxs, conn, &[0x8du8, 0x8fu8])?;
    let bts = super::struct2byte(&info);
    super::tcp_write(ctxs, conn, bts)?;
    if let Some(v) = &cmds {
        super::tcp_write(ctxs, conn, v.as_bytes())?;
    }
    if let Some(v) = &hds {
        super::tcp_write(ctxs, conn, &v[..])?;
    }
    if let Some(v) = &bds {
        super::tcp_write(ctxs, conn, &v[..])?;
    }
    super::tcp_write(ctxs, conn, &[0x8eu8, 0x8fu8])?;
    Ok(())
}

pub fn send_msg_buf(
    ctxs: &super::Context,
    conn: &mut std::net::TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: Option<Arc<Box<[u8]>>>,
    bds: Option<Arc<ByteBoxBuf>>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.lenCmd = v.len() as u16;
    }
    if let Some(v) = &hds {
        info.lenHead = v.len() as u32;
    }
    if let Some(v) = &bds {
        info.lenBody = v.len() as u32;
    }
    super::tcp_write(ctxs, conn, &[0x8du8, 0x8fu8])?;
    let bts = super::struct2byte(&info);
    super::tcp_write(ctxs, conn, bts)?;
    if let Some(v) = &cmds {
        super::tcp_write(ctxs, conn, v.as_bytes())?;
    }
    if let Some(v) = &hds {
        super::tcp_write(ctxs, conn, &v[..])?;
    }
    if let Some(v) = &bds {
        bytes::tcp_write(ctxs, conn, &*v)?;
    }
    super::tcp_write(ctxs, conn, &[0x8eu8, 0x8fu8])?;
    Ok(())
}

pub async fn parse_msg_async(
    ctxs: &super::Context,
    conn: &mut async_std::net::TcpStream,
) -> io::Result<Message> {
    let bts = super::tcp_read_async(ctxs, conn, 1).await?;
    if bts.len() < 1 && bts[0] != 0x8du8 {
        return Err(super::ioerr(format!("first byte err:{:?}", &bts[..]), None));
    }
    let bts = super::tcp_read_async(ctxs, conn, 1).await?;
    if bts.len() < 1 && bts[0] != 0x8fu8 {
        return Err(super::ioerr(format!("first byte err:{:?}", &bts[..]), None));
    }

    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let bts = super::tcp_read_async(ctxs, conn, infoln).await?;
    super::byte2struct(&mut info, &bts[..])?;
    if (info.lenHead) as u64 > MaxHeads {
        return Err(super::ioerr("bytes2 out limit!!", None));
    }
    if (info.lenBody) as u64 > MaxBodys {
        return Err(super::ioerr("bytes3 out limit!!", None));
    }

    let mut rt = Message::new();
    rt.version = info.version;
    rt.control = info.control;
    let lnsz = info.lenCmd as usize;
    if lnsz > 0 {
        let bts = super::tcp_read_async(&ctxs, conn, lnsz).await?;
        rt.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(super::ioerr("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.lenHead as usize;
    if lnsz > 0 {
        let bts = super::tcp_read_async(&ctxs, conn, lnsz as usize).await?;
        rt.heads = Some(bts);
    }
    let lnsz = info.lenBody as usize;
    if lnsz > 0 {
        let bts = super::tcp_read_async(&ctxs, conn, lnsz as usize).await?;
        rt.bodys = Some(bts);
    }
    let bts = super::tcp_read_async(ctxs, conn, 2).await?;
    if bts.len() < 2 && bts[0] != 0x8eu8 && bts[1] != 0x8fu8 {
        return Err(super::ioerr(format!("end byte err:{:?}", &bts[..]), None));
    }

    Ok(rt)
}

pub async fn send_msg_async(
    ctxs: &super::Context,
    conn: &mut async_std::net::TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: Option<Arc<Box<[u8]>>>,
    bds: Option<Arc<Box<[u8]>>>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.lenCmd = v.len() as u16;
    }
    if let Some(v) = &hds {
        info.lenHead = v.len() as u32;
    }
    if let Some(v) = &bds {
        info.lenBody = v.len() as u32;
    }
    super::tcp_write_async(ctxs, conn, &[0x8du8, 0x8fu8]).await?;
    let bts = super::struct2byte(&info);
    super::tcp_write_async(ctxs, conn, bts).await?;
    if let Some(v) = &cmds {
        super::tcp_write_async(ctxs, conn, v.as_bytes()).await?;
    }
    if let Some(v) = &hds {
        super::tcp_write_async(ctxs, conn, &v[..]).await?;
    }
    if let Some(v) = &bds {
        super::tcp_write_async(ctxs, conn, &v[..]).await?;
    }
    super::tcp_write_async(ctxs, conn, &[0x8eu8, 0x8fu8]).await?;
    Ok(())
}

pub async fn send_msg_async_buf(
    ctxs: &super::Context,
    conn: &mut async_std::net::TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: Option<Arc<Box<[u8]>>>,
    bds: Option<Arc<ByteBoxBuf>>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.lenCmd = v.len() as u16;
    }
    if let Some(v) = &hds {
        info.lenHead = v.len() as u32;
    }
    if let Some(v) = &bds {
        info.lenBody = v.len() as u32;
    }
    super::tcp_write_async(ctxs, conn, &[0x8du8, 0x8fu8]).await?;
    let bts = super::struct2byte(&info);
    super::tcp_write_async(ctxs, conn, bts).await?;
    if let Some(v) = &cmds {
        super::tcp_write_async(ctxs, conn, v.as_bytes()).await?;
    }
    if let Some(v) = &hds {
        super::tcp_write_async(ctxs, conn, &v[..]).await?;
    }
    if let Some(v) = &bds {
        bytes::tcp_write_async(ctxs, conn, &*v).await?;
    }
    super::tcp_write_async(ctxs, conn, &[0x8eu8, 0x8fu8]).await?;
    Ok(())
}
