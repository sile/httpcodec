/// Options for request/response decoders.
#[derive(Debug, Clone)]
pub struct DecodeOptions {
    /// The maximum number of bytes allowed for a start-line part.
    pub max_start_line_size: usize,

    /// The maximum number of bytes allowed for a header part.
    pub max_header_size: usize,
}
impl DecodeOptions {
    /// The default value of `max_start_line_size` field.
    pub const DEFAULT_MAX_START_LINE_SIZE: usize = 0xFFFF;

    /// The default value of `max_header_size` field.
    pub const DEFAULT_MAX_HEADER_SIZE: usize = 0xFFFF;
}
impl Default for DecodeOptions {
    fn default() -> Self {
        DecodeOptions {
            max_start_line_size: Self::DEFAULT_MAX_START_LINE_SIZE,
            max_header_size: Self::DEFAULT_MAX_HEADER_SIZE,
        }
    }
}
