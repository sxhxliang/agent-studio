//! Event Batching and Debouncing Support
//!
//! Provides utilities for batching multiple events and debouncing rapid event sequences.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Batched events with time window
pub struct BatchedEvents<T> {
    events: Vec<T>,
    last_flush: Instant,
    batch_size: usize,
    time_window: Duration,
}

impl<T> BatchedEvents<T>
where
    T: Clone,
{
    /// Create a new batched events collector
    ///
    /// - `batch_size`: Maximum number of events before auto-flush
    /// - `time_window`: Maximum time to wait before auto-flush
    pub fn new(batch_size: usize, time_window: Duration) -> Self {
        Self {
            events: Vec::with_capacity(batch_size),
            last_flush: Instant::now(),
            batch_size,
            time_window,
        }
    }

    /// Add an event to the batch
    ///
    /// Returns Some(Vec) with batched events if batch should be flushed, None otherwise.
    pub fn push(&mut self, event: T) -> Option<Vec<T>> {
        self.events.push(event);

        if self.should_flush() {
            Some(self.flush())
        } else {
            None
        }
    }

    /// Check if batch should be flushed
    fn should_flush(&self) -> bool {
        self.events.len() >= self.batch_size || self.last_flush.elapsed() >= self.time_window
    }

    /// Manually flush the batch
    pub fn flush(&mut self) -> Vec<T> {
        let events = std::mem::replace(&mut self.events, Vec::with_capacity(self.batch_size));
        self.last_flush = Instant::now();
        events
    }

    /// Get current batch size
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Debouncer for events - only emits after a quiet period
pub struct Debouncer<T> {
    last_event: Option<(T, Instant)>,
    quiet_period: Duration,
}

impl<T> Debouncer<T>
where
    T: Clone,
{
    /// Create a new debouncer
    ///
    /// - `quiet_period`: Time to wait after last event before emitting
    pub fn new(quiet_period: Duration) -> Self {
        Self {
            last_event: None,
            quiet_period,
        }
    }

    /// Add an event to the debouncer
    ///
    /// Returns Some(event) if the quiet period has elapsed, None otherwise.
    pub fn push(&mut self, event: T) -> Option<T> {
        let now = Instant::now();

        // Check if we should emit the previous event
        let should_emit = self
            .last_event
            .as_ref()
            .map(|(_, last_time)| now.duration_since(*last_time) >= self.quiet_period)
            .unwrap_or(false);

        let result = if should_emit {
            self.last_event.take().map(|(evt, _)| evt)
        } else {
            None
        };

        // Store the new event
        self.last_event = Some((event, now));

        result
    }

    /// Flush any pending event
    pub fn flush(&mut self) -> Option<T> {
        self.last_event.take().map(|(evt, _)| evt)
    }

    /// Check if there's a pending event
    pub fn has_pending(&self) -> bool {
        self.last_event.is_some()
    }
}

/// Thread-safe batched event collector
pub struct BatchedEventCollector<T> {
    inner: Arc<Mutex<BatchedEvents<T>>>,
}

impl<T> BatchedEventCollector<T>
where
    T: Clone,
{
    /// Create a new batched event collector
    pub fn new(batch_size: usize, time_window: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BatchedEvents::new(batch_size, time_window))),
        }
    }

    /// Add an event to the batch
    ///
    /// Returns Some(Vec) with batched events if batch should be flushed.
    pub fn push(&self, event: T) -> Option<Vec<T>> {
        let mut batched = self.inner.lock().unwrap();
        batched.push(event)
    }

    /// Manually flush the batch
    pub fn flush(&self) -> Vec<T> {
        let mut batched = self.inner.lock().unwrap();
        batched.flush()
    }

    /// Get current batch size
    pub fn len(&self) -> usize {
        let batched = self.inner.lock().unwrap();
        batched.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        let batched = self.inner.lock().unwrap();
        batched.is_empty()
    }
}

impl<T> Clone for BatchedEventCollector<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Thread-safe debouncer
pub struct DebouncerContainer<T> {
    inner: Arc<Mutex<Debouncer<T>>>,
}

impl<T> DebouncerContainer<T>
where
    T: Clone,
{
    /// Create a new debouncer container
    pub fn new(quiet_period: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Debouncer::new(quiet_period))),
        }
    }

    /// Add an event to the debouncer
    pub fn push(&self, event: T) -> Option<T> {
        let mut debouncer = self.inner.lock().unwrap();
        debouncer.push(event)
    }

    /// Flush any pending event
    pub fn flush(&self) -> Option<T> {
        let mut debouncer = self.inner.lock().unwrap();
        debouncer.flush()
    }

    /// Check if there's a pending event
    pub fn has_pending(&self) -> bool {
        let debouncer = self.inner.lock().unwrap();
        debouncer.has_pending()
    }
}

impl<T> Clone for DebouncerContainer<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_batched_events_size_limit() {
        let mut batched = BatchedEvents::new(3, Duration::from_secs(10));

        assert!(batched.push(1).is_none());
        assert!(batched.push(2).is_none());

        // Third event should trigger flush
        let flushed = batched.push(3);
        assert!(flushed.is_some());
        assert_eq!(flushed.unwrap(), vec![1, 2, 3]);
        assert_eq!(batched.len(), 0);
    }

    #[test]
    fn test_batched_events_time_window() {
        let mut batched = BatchedEvents::new(100, Duration::from_millis(50));

        batched.push(1);
        thread::sleep(Duration::from_millis(60));

        // Time window elapsed, should flush
        let flushed = batched.push(2);
        assert!(flushed.is_some());
    }

    #[test]
    fn test_batched_events_manual_flush() {
        let mut batched = BatchedEvents::new(10, Duration::from_secs(10));

        batched.push(1);
        batched.push(2);

        let flushed = batched.flush();
        assert_eq!(flushed, vec![1, 2]);
        assert_eq!(batched.len(), 0);
    }

    #[test]
    fn test_debouncer() {
        let mut debouncer = Debouncer::new(Duration::from_millis(50));

        // First event - no emission
        assert!(debouncer.push(1).is_none());

        thread::sleep(Duration::from_millis(60));

        // Quiet period elapsed - should emit first event
        let emitted = debouncer.push(2);
        assert_eq!(emitted, Some(1));

        // Flush remaining
        assert_eq!(debouncer.flush(), Some(2));
    }

    #[test]
    fn test_debouncer_rapid_events() {
        let mut debouncer = Debouncer::new(Duration::from_millis(100));

        // Rapid events - none should be emitted
        for i in 1..=5 {
            assert!(debouncer.push(i).is_none());
            thread::sleep(Duration::from_millis(10));
        }

        // Flush should return the last event
        assert_eq!(debouncer.flush(), Some(5));
    }

    #[test]
    fn test_batched_event_collector() {
        let collector = BatchedEventCollector::new(3, Duration::from_secs(10));

        assert!(collector.push(1).is_none());
        assert!(collector.push(2).is_none());

        let flushed = collector.push(3);
        assert!(flushed.is_some());
        assert_eq!(flushed.unwrap(), vec![1, 2, 3]);
        assert_eq!(collector.len(), 0);
    }

    #[test]
    fn test_debouncer_container() {
        let debouncer = DebouncerContainer::new(Duration::from_millis(50));

        assert!(debouncer.push(1).is_none());
        assert!(debouncer.has_pending());

        thread::sleep(Duration::from_millis(60));

        let emitted = debouncer.push(2);
        assert_eq!(emitted, Some(1));
        assert_eq!(debouncer.flush(), Some(2));
        assert!(!debouncer.has_pending());
    }
}
