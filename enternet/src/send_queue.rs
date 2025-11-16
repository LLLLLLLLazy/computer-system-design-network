use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
};

#[derive(Debug)]
pub enum SendQueueError {
    Closed,
    Full,
}

pub struct SendQueue {
    inner: Mutex<Inner>,
    event: Condvar,
}

struct Inner {
    capacity: usize,
    closed: bool,
    frames: VecDeque<Vec<u8>>,
}

impl SendQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                capacity,
                closed: false,
                frames: VecDeque::with_capacity(capacity),
            }),
            event: Condvar::new(),
        }
    }

    pub fn push(&self, frame: Vec<u8>) -> Result<(), SendQueueError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed {
            return Err(SendQueueError::Closed);
        }
        if inner.frames.len() >= inner.capacity {
            return Err(SendQueueError::Full);
        }
        inner.frames.push_back(frame);
        self.event.notify_one();
        Ok(())
    }

    pub fn pop(&self) -> Option<Vec<u8>> {
        let mut inner = self.inner.lock().unwrap();
        loop {
            if let Some(frame) = inner.frames.pop_front() {
                return Some(frame);
            }
            if inner.closed {
                return None;
            }
            inner = self.event.wait(inner).unwrap();
        }
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        self.event.notify_all();
    }
}