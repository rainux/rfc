use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const BUFFER_SIZE: usize = 8192; // ~186ms at 44.1kHz, generous

pub struct AudioBuffer {
    data: Box<[f32; BUFFER_SIZE]>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
}

impl AudioBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            data: Box::new([0.0; BUFFER_SIZE]),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
        })
    }

    pub fn push(&self, sample: f32) -> bool {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Acquire);
        let next_write = (write + 1) % BUFFER_SIZE;
        if next_write == read {
            return false; // Full
        }
        // Safety: single producer, only we write to data[write]
        unsafe {
            let ptr = self.data.as_ptr() as *mut f32;
            *ptr.add(write) = sample;
        }
        self.write_pos.store(next_write, Ordering::Release);
        true
    }

    pub fn pop(&self) -> Option<f32> {
        let read = self.read_pos.load(Ordering::Relaxed);
        let write = self.write_pos.load(Ordering::Acquire);
        if read == write {
            return None; // Empty
        }
        let sample = unsafe {
            let ptr = self.data.as_ptr();
            *ptr.add(read)
        };
        self.read_pos
            .store((read + 1) % BUFFER_SIZE, Ordering::Release);
        Some(sample)
    }

    /// How many samples are available to read
    pub fn len(&self) -> usize {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Relaxed);
        if write >= read {
            write - read
        } else {
            BUFFER_SIZE - read + write
        }
    }
}

// Safety: AudioBuffer uses atomic operations for synchronization and is designed
// for single-producer single-consumer use. Send+Sync is required for Arc sharing.
unsafe impl Send for AudioBuffer {}
unsafe impl Sync for AudioBuffer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let buf = AudioBuffer::new();
        assert_eq!(buf.len(), 0);
        assert!(buf.pop().is_none());

        assert!(buf.push(0.5));
        assert!(buf.push(-0.5));
        assert_eq!(buf.len(), 2);

        assert_eq!(buf.pop(), Some(0.5));
        assert_eq!(buf.pop(), Some(-0.5));
        assert_eq!(buf.pop(), None);
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_full_buffer() {
        let buf = AudioBuffer::new();
        // Fill buffer (capacity is BUFFER_SIZE - 1 due to ring buffer design)
        for i in 0..(BUFFER_SIZE - 1) {
            assert!(buf.push(i as f32), "push failed at {}", i);
        }
        assert_eq!(buf.len(), BUFFER_SIZE - 1);
        // Next push should fail
        assert!(!buf.push(999.0));
    }
}
