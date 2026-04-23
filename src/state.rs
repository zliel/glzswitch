use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Thread-safe application state for tracking current theme index.
///
/// Uses atomic operations to ensure safe concurrent access from multiple threads
/// (e.g., main thread and hotkey handler threads).
pub struct AppState {
    /// Atomically stores the current theme index.
    current_index: AtomicUsize,
    /// The total number of themes available.
    theme_count: usize,
    /// Timestamp of last theme switch (in milliseconds since epoch)
    last_switch: AtomicU64,
}

impl AppState {
    /// Creates a new AppState with the given number of themes.
    ///
    /// # Arguments
    /// * `theme_count` - Number of themes in the theme list
    ///
    /// # Panics
    /// None, but operations will panic if theme_count is 0 when calling advance()
    pub fn new(theme_count: usize) -> Self {
        AppState {
            current_index: AtomicUsize::new(0),
            theme_count,
            last_switch: AtomicU64::new(0),
        }
    }

    /// Check if we're still in cooldown period.
    ///
    /// # Arguments
    /// * `cooldown` - The cooldown duration to check against
    ///
    /// # Returns
    /// true if cooldown has not elapsed since last switch
    pub fn is_in_cooldown(&self, cooldown: Duration) -> bool {
        let last = self.last_switch.load(Ordering::SeqCst);
        if last == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let elapsed = now.saturating_sub(last);
        elapsed < u64::try_from(cooldown.as_millis()).unwrap_or(u64::MAX)
    }

    /// Record that a theme switch occurred.
    pub fn record_switch(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_switch.store(now, Ordering::SeqCst);
    }

    /// Calculates the next theme index without advancing.
    ///
    /// Wraps around to 0 when reaching theme_count.
    ///
    /// # Returns
    /// The next index that would be selected
    ///
    /// # Note
    /// Returns 0 if theme_count is 0 to avoid modulo by zero
    pub fn next_index(&self) -> usize {
        if self.theme_count == 0 {
            return 0;
        }
        let current = self.current_index.load(Ordering::SeqCst);
        (current + 1) % self.theme_count
    }

    /// Advances to the next theme index and returns it.
    ///
    /// # Returns
    /// The new current index after advancing
    ///
    /// # Note
    /// Returns 0 if theme_count is 0 to avoid undefined behavior
    pub fn advance(&self) -> usize {
        let next = self.next_index();
        self.current_index.store(next, Ordering::SeqCst);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_initial_index() {
        let state = AppState::new(5);
        assert_eq!(state.current_index.load(Ordering::SeqCst), 0);
        assert_eq!(state.theme_count, 5);
    }

    #[test]
    fn test_next_index_wraps() {
        let state = AppState::new(3);
        // Start at 0, next should be 1
        assert_eq!(state.next_index(), 1);

        // Simulate advancing twice
        state.advance();
        state.advance();
        assert_eq!(state.next_index(), 0); // wraps around
    }

    #[test]
    fn test_advance_updates_index() {
        let state = AppState::new(4);
        assert_eq!(state.advance(), 1);
        assert_eq!(state.advance(), 2);
        assert_eq!(state.advance(), 3);
        assert_eq!(state.advance(), 0); // wraps
    }

    #[test]
    fn test_zero_theme_count() {
        let state = AppState::new(0);
        assert_eq!(state.next_index(), 0);
        assert_eq!(state.advance(), 0);
    }
}
