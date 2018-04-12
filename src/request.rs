use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode};
use bytecodec::bytes::BytesEncoder;

use {ErrorKind, Result};
use token::PushByte;

// pub struct RequestLine;

#[derive(Debug)]
pub struct RequestTarget<B>(B);
impl<B: AsRef<[u8]>> AsRef<[u8]> for RequestTarget<B> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct RequestTargetDecoder<B>(B);
impl<B: PushByte> Decode for RequestTargetDecoder<B> {
    type Item = RequestTarget<B>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        for i in 0..buf.len() {
            if buf[i] == b' ' {
                let item = self.0.take_bytes();
                return Ok((i, Some(RequestTarget(item))));
            }
            track_assert!(self.0.push_byte(buf[i]), ErrorKind::Other, "Buffer full");
        }

        let item = if eos.is_reached() {
            Some(RequestTarget(self.0.take_bytes()))
        } else {
            None
        };
        Ok((buf.len(), item))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct RequestTargetEncoder<B>(BytesEncoder<RequestTarget<B>>);
impl<B: AsRef<[u8]>> Encode for RequestTargetEncoder<B> {
    type Item = RequestTarget<B>;

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
impl<B: AsRef<[u8]>> ExactBytesEncode for RequestTargetEncoder<B> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}
