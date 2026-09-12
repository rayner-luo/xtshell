/// 输出环形缓冲：保留最近 `cap` 字节，支持基于单调字节序号的增量读取。
pub struct OutputBuffer {
    buf: Vec<u8>,
    cap: usize,
    total_written: u64,
    start_offset: u64,
}

impl OutputBuffer {
    pub fn new(cap: usize) -> Self {
        OutputBuffer { buf: Vec::with_capacity(cap), cap, total_written: 0, start_offset: 0 }
    }

    pub fn append(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
        self.total_written += data.len() as u64;
        if self.buf.len() > self.cap {
            let excess = self.buf.len() - self.cap;
            self.buf.drain(..excess);
            self.start_offset += excess as u64;
        }
    }

    pub fn read_since(&self, offset: u64) -> (Vec<u8>, u64) {
        let end = self.total_written;
        let offset = offset.max(self.start_offset);
        let local = (offset - self.start_offset) as usize;
        let slice = if local < self.buf.len() { &self.buf[local..] } else { &[] };
        (slice.to_vec(), end)
    }

    pub fn tail(&self, n: usize) -> Vec<u8> {
        let start = self.buf.len().saturating_sub(n);
        self.buf[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_reads_increment() {
        let mut b = OutputBuffer::new(1024);
        b.append(b"hello");
        b.append(b" world");
        let (out, next) = b.read_since(0);
        assert_eq!(out, b"hello world");
        assert_eq!(next, 11);
        let (out2, next2) = b.read_since(next);
        assert_eq!(out2, b"");
        assert_eq!(next2, 11);
    }

    #[test]
    fn truncates_old_data_on_overflow() {
        let mut b = OutputBuffer::new(8);
        b.append(b"1234567890");
        let (out, _) = b.read_since(0);
        assert_eq!(out, b"34567890");
    }

    #[test]
    fn offset_before_window_clamps_to_start() {
        let mut b = OutputBuffer::new(8);
        b.append(b"abcdefghij");
        let (out, next) = b.read_since(2);
        assert_eq!(out, b"cdefghij");
        assert_eq!(next, 10);
    }

    #[test]
    fn zero_capacity_keeps_nothing() {
        let mut b = OutputBuffer::new(0);
        b.append(b"data");
        assert_eq!(b.read_since(0), (Vec::<u8>::new(), 4));
    }
}
