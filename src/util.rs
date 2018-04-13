use bytecodec::{ByteCount, Decode, Eos};
use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::combinator::Buffered;

use {ErrorKind, Result};

#[derive(Debug, Default)]
pub struct WithOwsDecoder<D: Decode> {
    is_front_ows_finished: bool,
    inner: Buffered<D>,
}
impl<D: Decode> Decode for WithOwsDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        for &b in buf {
            if self.is_front_ows_finished || !is_whitespace(b) {
                self.is_front_ows_finished = true;
                break;
            }
            offset += 1;
        }
        if !self.is_front_ows_finished {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            return Ok((offset, None));
        }

        if !self.inner.has_item() {
            offset += track!(self.inner.decode(&buf[offset..], eos))?.0;
            if !self.inner.has_item() {
                return Ok((offset, None));
            }
        }

        for &b in &buf[offset..] {
            if !is_whitespace(b) {
                self.is_front_ows_finished = false;
                return Ok((offset, self.inner.take_item()));
            }
            offset += 1;
        }
        Ok((offset, None))
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct WithSpDecoder<D: Decode> {
    inner: Buffered<D>,
    space: CopyableBytesDecoder<[u8; 1]>,
}
impl<D: Decode> Decode for WithSpDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.inner.has_item() {
            offset += track!(self.inner.decode(buf, eos))?.0;
        }
        if self.inner.has_item() {
            let (size, item) = track!(self.space.decode(&buf[offset..], eos))?;
            offset += size;
            if let Some(b) = item {
                track_assert_eq!(b[0], b' ', ErrorKind::InvalidInput);
            }
            let item = item.and_then(|_| self.inner.take_item());
            Ok((offset, item))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct WithColonDecoder<D: Decode> {
    inner: Buffered<D>,
    space: CopyableBytesDecoder<[u8; 1]>,
}
impl<D: Decode> Decode for WithColonDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.inner.has_item() {
            offset += track!(self.inner.decode(buf, eos))?.0;
        }
        if self.inner.has_item() {
            let (size, item) = track!(self.space.decode(&buf[offset..], eos))?;
            offset += size;
            if let Some(b) = item {
                track_assert_eq!(b[0], b':', ErrorKind::InvalidInput);
            }
            let item = item.and_then(|_| self.inner.take_item());
            Ok((offset, item))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct WithCrlfDecoder<D: Decode> {
    inner: Buffered<D>,
    crlf: CopyableBytesDecoder<[u8; 2]>,
}
impl<D: Decode> Decode for WithCrlfDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.inner.has_item() {
            offset += track!(self.inner.decode(buf, eos))?.0;
        }
        if self.inner.has_item() {
            let (size, item) = track!(self.crlf.decode(&buf[offset..], eos))?;
            offset += size;
            if let Some(b) = item {
                track_assert_eq!(b, [b'\r', b'\n'], ErrorKind::InvalidInput);
            }
            let item = item.and_then(|_| self.inner.take_item());
            Ok((offset, item))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO
        ByteCount::Unknown
    }
}

pub fn is_whitespace(b: u8) -> bool {
    match b {
        b' ' | b'\t' => true,
        _ => false,
    }
}
