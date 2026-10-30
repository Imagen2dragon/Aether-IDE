use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FramingError {
    #[error("Header is malformed: {0}")]
    MalformedHeader(String),
    #[error("Invalid Content-Length value: {0}")]
    InvalidContentLength(String),
    #[error("Missing Content-Length header")]
    MissingContentLength,
    #[error("Payload exceeded maximum buffer limit ({0} bytes)")]
    PayloadTooLarge(usize),
}

pub const MAX_PAYLOAD_SIZE: usize = 64 * 1024 * 1024; // 64 MB guard

pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    let mut out = Vec::with_capacity(header.len() + payload.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(payload);
    out
}

#[derive(Default, Debug)]
pub struct LspFrameDecoder {
    buffer: Vec<u8>,
}

impl LspFrameDecoder {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn accept_chunk(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    /// Attempts to decode the next full frame from the internal buffer.
    /// Returns `Ok(Some(payload))` if a complete message is ready,
    /// `Ok(None)` if more bytes are needed,
    /// or `Err(FramingError)` if header parsing fails.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, FramingError> {
        let buf = &self.buffer;

        // Search for header separator: "\r\n\r\n" or fallback "\n\n"
        let (header_end, body_start) = if let Some(pos) = find_subslice(buf, b"\r\n\r\n") {
            (pos, pos + 4)
        } else if let Some(pos) = find_subslice(buf, b"\n\n") {
            (pos, pos + 2)
        } else {
            return Ok(None);
        };

        let header_slice = &buf[..header_end];
        let header_str = std::str::from_utf8(header_slice)
            .map_err(|e| FramingError::MalformedHeader(format!("Non-UTF8 headers: {}", e)))?;

        let mut content_length = None;

        for line in header_str.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((name, val)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    let val_str = val.trim();
                    let len: usize = val_str
                        .parse()
                        .map_err(|_| FramingError::InvalidContentLength(val_str.to_string()))?;
                    content_length = Some(len);
                }
            }
        }

        let length = match content_length {
            Some(l) => l,
            None => return Err(FramingError::MissingContentLength),
        };

        if length > MAX_PAYLOAD_SIZE {
            return Err(FramingError::PayloadTooLarge(length));
        }

        let needed_total = body_start + length;
        if buf.len() < needed_total {
            // Need more data for the full payload
            return Ok(None);
        }

        let payload = buf[body_start..needed_total].to_vec();
        // Consume processed frame from buffer
        self.buffer.drain(..needed_total);

        Ok(Some(payload))
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_and_decode_single_frame() {
        let payload = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}";
        let encoded = encode_frame(payload);

        let mut decoder = LspFrameDecoder::new();
        decoder.accept_chunk(&encoded);

        let decoded = decoder.next_frame().unwrap().expect("frame should decode");
        assert_eq!(decoded, payload);
        assert!(decoder.next_frame().unwrap().is_none());
        assert!(decoder.is_empty());
    }

    #[test]
    fn test_glued_frames() {
        let p1 = b"{\"id\":1}";
        let p2 = b"{\"id\":2}";

        let mut stream = encode_frame(p1);
        stream.extend_from_slice(&encode_frame(p2));

        let mut decoder = LspFrameDecoder::new();
        decoder.accept_chunk(&stream);

        let d1 = decoder.next_frame().unwrap().expect("first frame");
        assert_eq!(d1, p1);

        let d2 = decoder.next_frame().unwrap().expect("second frame");
        assert_eq!(d2, p2);

        assert!(decoder.next_frame().unwrap().is_none());
    }

    #[test]
    fn test_partial_chunks() {
        let p = b"{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\"}";
        let encoded = encode_frame(p);

        let mut decoder = LspFrameDecoder::new();

        // Feed half of the frame
        decoder.accept_chunk(&encoded[..15]);
        assert!(decoder.next_frame().unwrap().is_none());

        // Feed rest
        decoder.accept_chunk(&encoded[15..]);
        let d = decoder
            .next_frame()
            .unwrap()
            .expect("full frame now available");
        assert_eq!(d, p);
    }

    #[test]
    fn test_malformed_header_handling() {
        let mut decoder = LspFrameDecoder::new();
        decoder.accept_chunk(b"Invalid-Header\r\n\r\n{}");
        let res = decoder.next_frame();
        assert_eq!(res, Err(FramingError::MissingContentLength));
    }

    #[test]
    fn test_invalid_content_length() {
        let mut decoder = LspFrameDecoder::new();
        decoder.accept_chunk(b"Content-Length: not_a_number\r\n\r\n{}");
        let res = decoder.next_frame();
        assert!(matches!(res, Err(FramingError::InvalidContentLength(_))));
    }
}
