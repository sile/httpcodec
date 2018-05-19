use bytecodec::bytes::BytesEncoder;
use bytecodec::combinator::{MaxBytes, Peekable};
use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, ErrorKind, Result, SizedEncode};
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
    start_line: MaxBytes<S>,
    header: Peekable<MaxBytes<HeaderDecoder>>,
    body: B,
    options: DecodeOptions,
}
impl<S: Decode, B: BodyDecode> MessageDecoder<S, B> {
    pub fn new(start_line: S, body: B, options: DecodeOptions) -> Self {
        MessageDecoder {
            buf: Vec::new(),
            start_line: start_line.max_bytes(options.max_start_line_size as u64),
            header: HeaderDecoder::default()
                .max_bytes(options.max_header_size as u64)
                .peekable(),
            body,
            options,
        }
    }
}
impl<S: Decode, B: BodyDecode> Decode for MessageDecoder<S, B> {
    type Item = Message<S::Item, B::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        let mut offset = 0;
        if !self.start_line.is_idle() {
            offset += track!(self.start_line.decode(buf, eos))?;
            if !self.start_line.is_idle() {
                self.buf.extend_from_slice(&buf[..offset]);
                return Ok(offset);
            } else {
                self.header
                    .inner_mut()
                    .inner_mut()
                    .set_start_position(self.buf.len() + offset);
            }
        }

        if !self.header.is_idle() {
            offset += track!(self.header.decode(&buf[offset..], eos))?;
            self.buf.extend_from_slice(&buf[..offset]);
            if let Some(header) = self.header.peek() {
                track!(self.body.initialize(&Header::new(&self.buf, header)))?;
            } else {
                return Ok(offset);
            }
        }

        bytecodec_try_decode!(self.body, offset, buf, eos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let body = track!(self.body.finish_decoding())?;
        let buf = mem::replace(&mut self.buf, Vec::new());
        let start_line = track!(self.start_line.finish_decoding())?;
        let header = track!(self.header.finish_decoding())?;
        Ok(Message {
            buf,
            start_line,
            header,
            body,
        })
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.header
            .requiring_bytes()
            .add_for_decoding(self.body.requiring_bytes())
    }

    fn is_idle(&self) -> bool {
        self.header.is_idle() && self.body.is_idle()
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
impl<B: SizedEncode + BodyEncode> SizedEncode for MessageEncoder<B> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.before_body.exact_requiring_bytes() + self.body.exact_requiring_bytes()
    }
}
