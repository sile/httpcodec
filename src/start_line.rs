use {HttpVersion, Method, RequestTarget};

#[derive(Debug)]
pub enum StartLine<'a> {
    Request {
        method: Method<'a>,
        target: RequestTarget<'a>,
        version: HttpVersion,
    },
    // TODO
    // Status{
    // }
}
