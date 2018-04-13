use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode};
use bytecodec::bytes::BytesEncoder;
use bytecodec::combinator::Buffered;

use {ErrorKind, Result};
use token::PushByte;
use util::{WithCrlfDecoder, WithSpDecoder};

#[derive(Debug)]
pub struct RequestLine<M, T, V> {
    pub method: M,
    pub request_target: T,
    pub http_version: V,
}

#[derive(Debug, Default)]
pub struct RequestLineDecoder<M, T, V>
where
    M: Decode,
    T: Decode,
    V: Decode,
{
    method: Buffered<WithSpDecoder<M>>,
    request_target: Buffered<WithSpDecoder<T>>,
    http_version: WithCrlfDecoder<V>,
}
impl<M, T, V> Decode for RequestLineDecoder<M, T, V>
where
    M: Decode,
    T: Decode,
    V: Decode,
{
    type Item = RequestLine<M::Item, T::Item, V::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.method.has_item() {
            offset += track!(self.method.decode(buf, eos))?.0;
            if !self.method.has_item() {
                return Ok((offset, None));
            }
        }
        if !self.request_target.has_item() {
            offset += track!(self.request_target.decode(&buf[offset..], eos))?.0;
            if !self.request_target.has_item() {
                return Ok((offset, None));
            }
        }
        let (size, item) = track!(self.http_version.decode(&buf[offset..], eos))?;
        offset += size;
        if let Some(http_version) = item {
            let method = self.method.take_item().expect("Never fails");
            let request_target = self.request_target.take_item().expect("Never fails");
            let request_line = RequestLine {
                method,
                request_target,
                http_version,
            };
            Ok((offset, Some(request_line)))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.method.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO:
        ByteCount::Unknown
    }
}

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
