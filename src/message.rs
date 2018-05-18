use bytecodec::bytes::BytesEncoder;
use bytecodec::combinator::{Buffered, MaxBytes};
use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, ErrorKind, ExactBytesEncode, Result};
use std::mem;

use body::{BodyDecode, BodyEncode};
use header::{Header, HeaderDecoder, HeaderFieldPosition, HeaderMut};
use options::DecodeOptions;

#[derive(Debug)]
pub struct Message<S, B> {
    pub buf: Vec<u8>,
    pub start_line: S,
    pub header: Vec<HeaderFieldPosition>,
    pub body: B,
}

#[derive(Debug)]
pub struct MessageDecoder<S: Decode, B> {
    buf: Vec<u8>,
    start_line: Buffered<MaxBytes<S>>,
    header: Buffered<MaxBytes<HeaderDecoder>>,
    body: B,
    options: DecodeOptions,
}
impl<S: Decode, B: BodyDecode> MessageDecoder<S, B> {
    pub fn new(start_line: S, body: B, options: DecodeOptions) -> Self {
        MessageDecoder {
            buf: Vec::new(),
            start_line: start_line
                .max_bytes(options.max_start_line_size as u64)
                .buffered(),
            header: HeaderDecoder::default()
                .max_bytes(options.max_header_size as u64)
                .buffered(),
            body,
            options,
        }
    }
}
impl<S: Decode, B: BodyDecode> Decode for MessageDecoder<S, B> {
    type Item = Message<S::Item, B::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.start_line.has_item() {
            offset = track!(self.start_line.decode(buf, eos))?.0;
            if !self.start_line.has_item() {
                self.buf.extend_from_slice(&buf[..offset]);
                return Ok((offset, None));
            } else {
                self.header
                    .inner_mut()
                    .inner_mut()
                    .set_start_position(self.buf.len() + offset);
            }
        }

        if !self.header.has_item() {
            offset += track!(self.header.decode(&buf[offset..], eos))?.0;
            self.buf.extend_from_slice(&buf[..offset]);
            if let Some(header) = self.header.get_item() {
                track!(self.body.initialize(&Header::new(&self.buf, header)))?;
            } else {
                return Ok((offset, None));
            }
        }

        let (size, item) = track!(self.body.decode(&buf[offset..], eos))?;
        offset += size;
        let item = item.map(|body| {
            let buf = mem::replace(&mut self.buf, Vec::new());
            let start_line = self.start_line.take_item().expect("Never fails");
            let header = self.header.take_item().expect("Never fails");
            Message {
                buf,
                start_line,
                header,
                body,
            }
        });
        Ok((offset, item))
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.header.has_item() {
            self.body.requiring_bytes()
        } else {
            ByteCount::Unknown
        }
    }
}

#[derive(Debug, Default)]
pub struct MessageEncoder<B> {
    before_body: BytesEncoder<Vec<u8>>,
    body: B,
}
impl<B: BodyEncode> MessageEncoder<B> {
    pub fn new(body: B) -> Self {
        MessageEncoder {
            before_body: BytesEncoder::new(),
            body,
        }
    }
}
impl<B: BodyEncode> Encode for MessageEncoder<B> {
    type Item = Message<(), B::Item>;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        let mut offset = 0;
        if !self.before_body.is_idle() {
            offset += track!(self.before_body.encode(buf, eos))?;
            if !self.before_body.is_idle() {
                return Ok(offset);
            }
        }
        offset += track!(self.body.encode(&mut buf[offset..], eos))?;
        Ok(offset)
    }

    fn start_encoding(&mut self, mut item: Self::Item) -> Result<()> {
        track_assert!(self.is_idle(), ErrorKind::EncoderFull);
        track!(self.body.start_encoding(item.body))?;
        {
            let mut header = HeaderMut::new(&mut item.buf, &mut item.header);
            track!(self.body.update_header(&mut header))?;
        }
        item.buf.extend_from_slice(b"\r\n");
        track!(self.before_body.start_encoding(item.buf))?;
        Ok(())
    }

    fn is_idle(&self) -> bool {
        self.before_body.is_idle() && self.body.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.before_body
            .requiring_bytes()
            .add_for_encoding(self.body.requiring_bytes())
    }
}
impl<B: ExactBytesEncode + BodyEncode> ExactBytesEncode for MessageEncoder<B> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.before_body.exact_requiring_bytes() + self.body.exact_requiring_bytes()
    }
}
