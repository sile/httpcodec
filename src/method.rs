use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode};
use bytecodec::bytes::BytesEncoder;

use {ErrorKind, Result};
use token::{Bytes, PushByte, Token, TokenDecoder, TokenEncoder};

#[derive(Debug)]
pub struct Method<B>(Token<B>);

#[derive(Debug, Default)]
pub struct MethodDecoder<B>(TokenDecoder<B>);
impl<B: PushByte> Decode for MethodDecoder<B> {
    type Item = Method<B>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        Ok((size, item.map(Method)))
    }

    fn has_terminated(&self) -> bool {
        self.0.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}

#[derive(Debug, Default)]
pub struct MethodEncoder<B>(TokenEncoder<B>);
impl<B: AsRef<[u8]>> Encode for MethodEncoder<B> {
    type Item = Method<B>;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        track!(self.0.encode(buf, eos))
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self.0.start_encoding(item.0))
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl<B: AsRef<[u8]>> ExactBytesEncode for MethodEncoder<B> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}

/// https://www.iana.org/assignments/http-methods/http-methods.xhtml
#[derive(Debug)]
pub enum IanaMethod {
    Acl,
    BaselineControl,
    Bind,
    Checkin,
    Checkout,
    Connect,
    Copy,
    Delete,
    Get,
    Head,
    Label,
    Link,
    Lock,
    Merge,
    Mkactivity,
    Mkcalendar,
    Mkcol,
    Mkredirectref,
    Mkworkspace,
    Move,
    Options,
    Orderpatch,
    Patch,
    Post,
    Pri,
    Propfind,
    Proppatch,
    Put,
    Rebind,
    Report,
    Search,
    Trace,
    Unbind,
    Uncheckout,
    Unlink,
    Unlock,
    Update,
    Updateredirectref,
    VersionControl,
}
impl AsRef<[u8]> for IanaMethod {
    fn as_ref(&self) -> &[u8] {
        match *self {
            IanaMethod::Acl => b"ACL",
            IanaMethod::BaselineControl => b"BASELINE-CONTROL",
            IanaMethod::Bind => b"BIND",
            IanaMethod::Checkin => b"CHECKIN",
            IanaMethod::Checkout => b"CHECKOUT",
            IanaMethod::Connect => b"CONNECT",
            IanaMethod::Copy => b"COPY",
            IanaMethod::Delete => b"DELETE",
            IanaMethod::Get => b"GET",
            IanaMethod::Head => b"HEAD",
            IanaMethod::Label => b"LABEL",
            IanaMethod::Link => b"LINK",
            IanaMethod::Lock => b"LOCK",
            IanaMethod::Merge => b"MERGE",
            IanaMethod::Mkactivity => b"MKACTIVITY",
            IanaMethod::Mkcalendar => b"MKCALENDAR",
            IanaMethod::Mkcol => b"MKCOL",
            IanaMethod::Mkredirectref => b"MKREDIRECTREF",
            IanaMethod::Mkworkspace => b"MKWORKSPACE",
            IanaMethod::Move => b"MOVE",
            IanaMethod::Options => b"OPTIONS",
            IanaMethod::Orderpatch => b"ORDERPATCH",
            IanaMethod::Patch => b"PATCH",
            IanaMethod::Post => b"POST",
            IanaMethod::Pri => b"PRI",
            IanaMethod::Propfind => b"PROPFIND",
            IanaMethod::Proppatch => b"PROPPATCH",
            IanaMethod::Put => b"PUT",
            IanaMethod::Rebind => b"REBIND",
            IanaMethod::Report => b"REPORT",
            IanaMethod::Search => b"SEARCH",
            IanaMethod::Trace => b"TRACE",
            IanaMethod::Unbind => b"UNBIND",
            IanaMethod::Uncheckout => b"UNCHECKOUT",
            IanaMethod::Unlink => b"UNLINK",
            IanaMethod::Unlock => b"UNLOCK",
            IanaMethod::Update => b"UPDATE",
            IanaMethod::Updateredirectref => b"UPDATEREDIRECTREF",
            IanaMethod::VersionControl => b"VERSION-CONTROL",
        }
    }
}

#[derive(Debug, Default)]
pub struct IanaMethodDecoder(TokenDecoder<Bytes<[u8; 17]>>);
impl Decode for IanaMethodDecoder {
    type Item = IanaMethod;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        if let Some(m) = item {
            let method = match m.as_ref() {
                b"ACL" => IanaMethod::Acl,
                b"BASELINE-CONTROL" => IanaMethod::BaselineControl,
                b"BIND" => IanaMethod::Bind,
                b"CHECKIN" => IanaMethod::Checkin,
                b"CHECKOUT" => IanaMethod::Checkout,
                b"CONNECT" => IanaMethod::Connect,
                b"COPY" => IanaMethod::Copy,
                b"DELETE" => IanaMethod::Delete,
                b"GET" => IanaMethod::Get,
                b"HEAD" => IanaMethod::Head,
                b"LABEL" => IanaMethod::Label,
                b"LINK" => IanaMethod::Link,
                b"LOCK" => IanaMethod::Lock,
                b"MERGE" => IanaMethod::Merge,
                b"MKACTIVITY" => IanaMethod::Mkactivity,
                b"MKCALENDAR" => IanaMethod::Mkcalendar,
                b"MKCOL" => IanaMethod::Mkcol,
                b"MKREDIRECTREF" => IanaMethod::Mkredirectref,
                b"MKWORKSPACE" => IanaMethod::Mkworkspace,
                b"MOVE" => IanaMethod::Move,
                b"OPTIONS" => IanaMethod::Options,
                b"ORDERPATCH" => IanaMethod::Orderpatch,
                b"PATCH" => IanaMethod::Patch,
                b"POST" => IanaMethod::Post,
                b"PRI" => IanaMethod::Pri,
                b"PROPFIND" => IanaMethod::Propfind,
                b"PROPPATCH" => IanaMethod::Proppatch,
                b"PUT" => IanaMethod::Put,
                b"REBIND" => IanaMethod::Rebind,
                b"REPORT" => IanaMethod::Report,
                b"SEARCH" => IanaMethod::Search,
                b"TRACE" => IanaMethod::Trace,
                b"UNBIND" => IanaMethod::Unbind,
                b"UNCHECKOUT" => IanaMethod::Uncheckout,
                b"UNLINK" => IanaMethod::Unlink,
                b"UNLOCK" => IanaMethod::Unlock,
                b"UPDATE" => IanaMethod::Update,
                b"UPDATEREDIRECTREF" => IanaMethod::Updateredirectref,
                b"VERSION-CONTROL" => IanaMethod::VersionControl,
                _ => track_panic!(ErrorKind::InvalidInput, "Unregistered HTTP method: {:?}", m),
            };
            Ok((size, Some(method)))
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
pub struct IanaMethodEncoder(BytesEncoder<IanaMethod>);
impl Encode for IanaMethodEncoder {
    type Item = IanaMethod;

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
impl ExactBytesEncode for IanaMethodEncoder {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}
