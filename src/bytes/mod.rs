// pub use bytes::ByteBox;
pub use ::bytes::Buf;
pub use ::bytes::BufMut;
pub use ::bytes::Bytes;
pub use ::bytes::BytesMut;
pub use bytes::ByteBoxBuf;
pub use circle::CircleBuf;
#[cfg(any(feature = "asyncs", feature = "tokios"))]
pub use stream::ByteSteamBuf;

mod bytes;
mod circle;
#[cfg(any(feature = "asyncs", feature = "tokios"))]
mod stream;

pub fn conv_human_readable(sz: u64) -> String {
    let szf = sz as f64;
    if sz >= 1024 * 1024 * 1024 {
        return format!("{:.2}GB", szf / 1024f64 / 1024f64 / 1024f64);
    } else if sz >= 1024 * 1024 {
        return format!("{:.2}MB", szf / 1024f64 / 1024f64);
    } else if sz >= 1024 {
        return format!("{:.2}KB", szf / 1024f64);
    }
    return format!("{}B", sz);
}

#[cfg(test)]
mod tests {
    use super::ByteBoxBuf;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn bufs() {
        let mut buf = ByteBoxBuf::new();
        buf.push(vec![0x8e, 0x8f]);
        buf.push(vec![0xff, 0xa7, 0x33]);
        let bts = buf.to_bytes();
        println!("datas({}/{}):{:?}", bts.len(), buf.len(), &bts[..]);
    }
}
