use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode};
use bytecodec::bytes::{BytesEncoder, CopyableBytesDecoder};

use {ErrorKind, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HttpVersion {
    V1_0,
    V1_1,
}
impl AsRef<[u8]> for HttpVersion {
    fn as_ref(&self) -> &[u8] {
        match *self {
            HttpVersion::V1_0 => b"HTTP/1.0",
            HttpVersion::V1_1 => b"HTTP/1.1",
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct HttpVersionDecoder(CopyableBytesDecoder<[u8; 10]>);
impl Decode for HttpVersionDecoder {
    type Item = HttpVersion;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        if let Some(v) = item {
            let v = match v.as_ref() {
                b"HTTP/1.0\r\n" => HttpVersion::V1_0,
                b"HTTP/1.1\r\n" => HttpVersion::V1_1,
                _ => track_panic!(ErrorKind::InvalidInput, "Unknown HTTP version: {:?}", v),
            };
            Ok((size, Some(v)))
        } else {
            Ok((size, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.0.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}

#[derive(Debug, Default)]
pub struct HttpVersionEncoder(BytesEncoder<HttpVersion>);
impl Encode for HttpVersionEncoder {
    type Item = HttpVersion;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        track!(self.0.encode(buf, eos))
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self.0.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl ExactBytesEncode for HttpVersionEncoder {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}
