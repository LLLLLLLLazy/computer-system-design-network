use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
};

#[derive(Debug)]
pub enum QueueError {
    Closed,
    Full,
}

pub struct RecvQueue {
    inner: Mutex<Inner>,
    event: Condvar,
}

struct Inner {
    capacity: usize,
    closed: bool,
    data: VecDeque<Vec<u8>>,
}

impl RecvQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                capacity,
                closed: false,
                data: VecDeque::with_capacity(capacity),
            }),
            event: Condvar::new(),
        }
    }

    pub fn push(&self, frame: Vec<u8>) -> Result<(), QueueError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed {
            return Err(QueueError::Closed);
        }
        if inner.data.len() >= inner.capacity {
            return Err(QueueError::Full);
        }
        inner.data.push_back(frame);
        self.event.notify_one();
        Ok(())
    }

    pub fn pop(&self) -> Option<Vec<u8>> {
        let mut inner = self.inner.lock().unwrap();
        loop {
            if let Some(frame) = inner.data.pop_front() {
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