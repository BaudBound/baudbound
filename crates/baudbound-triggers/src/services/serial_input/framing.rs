use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum FrameEvent {
    Message(Vec<u8>),
    Oversized,
}

#[derive(Debug)]
pub(super) struct LineFramer {
    buffer: Vec<u8>,
    discarding: bool,
    max_message_bytes: usize,
    swallow_lf: bool,
}

impl LineFramer {
    pub(super) fn new(max_message_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            discarding: false,
            max_message_bytes,
            swallow_lf: false,
        }
    }

    pub(super) fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    pub(super) fn push(&mut self, bytes: &[u8]) -> Vec<FrameEvent> {
        let mut events = Vec::new();
        for &byte in bytes {
            match byte {
                b'\r' => {
                    self.finish_line(&mut events);
                    self.swallow_lf = true;
                }
                b'\n' if self.swallow_lf => self.swallow_lf = false,
                b'\n' => self.finish_line(&mut events),
                _ => {
                    self.swallow_lf = false;
                    if self.discarding {
                        continue;
                    }
                    if self.buffer.len() == self.max_message_bytes {
                        self.buffer.clear();
                        self.discarding = true;
                        events.push(FrameEvent::Oversized);
                    } else {
                        self.buffer.push(byte);
                    }
                }
            }
        }
        events
    }

    fn finish_line(&mut self, events: &mut Vec<FrameEvent>) {
        if self.discarding {
            self.discarding = false;
            self.buffer.clear();
            return;
        }
        if !self.buffer.is_empty() {
            events.push(FrameEvent::Message(std::mem::take(&mut self.buffer)));
        }
    }
}

#[derive(Debug)]
pub(super) struct IdleGapFramer {
    buffer: Vec<u8>,
    discarding: bool,
    gap: Duration,
    last_byte_at: Option<Instant>,
    max_message_bytes: usize,
}

impl IdleGapFramer {
    pub(super) fn new(gap: Duration, max_message_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            discarding: false,
            gap,
            last_byte_at: None,
            max_message_bytes,
        }
    }

    pub(super) fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    pub(super) fn push(&mut self, bytes: &[u8], now: Instant) -> Option<FrameEvent> {
        if bytes.is_empty() {
            return None;
        }
        self.last_byte_at = Some(now);
        if self.discarding {
            return None;
        }
        if self.buffer.len().saturating_add(bytes.len()) > self.max_message_bytes {
            self.buffer.clear();
            self.discarding = true;
            return Some(FrameEvent::Oversized);
        }
        self.buffer.extend_from_slice(bytes);
        None
    }

    pub(super) fn poll(&mut self, now: Instant) -> Option<FrameEvent> {
        let last_byte_at = self.last_byte_at?;
        if now.saturating_duration_since(last_byte_at) < self.gap {
            return None;
        }
        self.last_byte_at = None;
        if self.discarding {
            self.discarding = false;
            self.buffer.clear();
            return None;
        }
        (!self.buffer.is_empty()).then(|| FrameEvent::Message(std::mem::take(&mut self.buffer)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_mode_accepts_all_common_endings_without_trimming_content() {
        let mut framer = LineFramer::new(1024);
        assert_eq!(
            framer.push(b" first \rsecond\nthird\r\nfourth\r"),
            vec![
                FrameEvent::Message(b" first ".to_vec()),
                FrameEvent::Message(b"second".to_vec()),
                FrameEvent::Message(b"third".to_vec()),
                FrameEvent::Message(b"fourth".to_vec()),
            ]
        );
    }

    #[test]
    fn line_mode_handles_split_crlf_and_ignores_empty_lines() {
        let mut framer = LineFramer::new(1024);
        assert_eq!(
            framer.push(b"message\r"),
            vec![FrameEvent::Message(b"message".to_vec())]
        );
        assert!(framer.push(b"\n\r\n\n").is_empty());
    }

    #[test]
    fn line_mode_recovers_after_oversized_message() {
        let mut framer = LineFramer::new(4);
        assert_eq!(framer.push(b"12345"), vec![FrameEvent::Oversized]);
        assert!(framer.push(b"ignored\n").is_empty());
        assert_eq!(
            framer.push(b"okay\n"),
            vec![FrameEvent::Message(b"okay".to_vec())]
        );
    }

    #[test]
    fn line_mode_does_not_emit_an_incomplete_message() {
        let mut framer = LineFramer::new(1024);

        assert!(framer.push(b"incomplete message").is_empty());
        assert_eq!(framer.buffered_bytes(), 18);
    }

    #[test]
    fn idle_gap_dispatches_after_silence_and_resets_after_each_chunk() {
        let start = Instant::now();
        let mut framer = IdleGapFramer::new(Duration::from_millis(100), 1024);
        assert_eq!(framer.push(b"hel", start), None);
        assert_eq!(framer.push(b"lo", start + Duration::from_millis(80)), None);
        assert_eq!(framer.poll(start + Duration::from_millis(150)), None);
        assert_eq!(
            framer.poll(start + Duration::from_millis(180)),
            Some(FrameEvent::Message(b"hello".to_vec()))
        );
    }

    #[test]
    fn idle_gap_recovers_after_oversized_message_at_next_gap() {
        let start = Instant::now();
        let mut framer = IdleGapFramer::new(Duration::from_millis(10), 4);
        assert_eq!(framer.push(b"12345", start), Some(FrameEvent::Oversized));
        assert_eq!(
            framer.push(b"ignored", start + Duration::from_millis(1)),
            None
        );
        assert_eq!(framer.poll(start + Duration::from_millis(11)), None);
        assert_eq!(
            framer.push(b"okay", start + Duration::from_millis(12)),
            None
        );
        assert_eq!(
            framer.poll(start + Duration::from_millis(22)),
            Some(FrameEvent::Message(b"okay".to_vec()))
        );
    }

    #[test]
    fn idle_gap_does_not_emit_an_incomplete_message() {
        let start = Instant::now();
        let mut framer = IdleGapFramer::new(Duration::from_millis(100), 1024);

        assert_eq!(framer.push(b"incomplete", start), None);
        assert_eq!(framer.poll(start + Duration::from_millis(99)), None);
        assert_eq!(framer.buffered_bytes(), 10);
    }
}
