use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const DELAYS_SECS: &[u64] = &[2, 5, 10, 30];

/// Política de reconexão com backoff exponencial
pub struct ReconnectPolicy {
    attempt: AtomicU32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self { attempt: AtomicU32::new(0) }
    }
}

impl ReconnectPolicy {
    pub fn attempt(&self) -> u32 {
        self.attempt.load(Ordering::Relaxed)
    }

    /// Retorna o delay para a próxima tentativa e incrementa o contador
    pub fn next_delay(&self) -> Duration {
        let current = self.attempt.fetch_add(1, Ordering::Relaxed) as usize;
        let secs = DELAYS_SECS.get(current).copied().unwrap_or(30);
        Duration::from_secs(secs)
    }

    pub fn reset(&self) {
        self.attempt.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_progresses_correctly() {
        let p = ReconnectPolicy::default();
        assert_eq!(p.next_delay(), Duration::from_secs(2));
        assert_eq!(p.next_delay(), Duration::from_secs(5));
        assert_eq!(p.next_delay(), Duration::from_secs(10));
        assert_eq!(p.next_delay(), Duration::from_secs(30));
        assert_eq!(p.next_delay(), Duration::from_secs(30)); // clampa em 30
    }

    #[test]
    fn reset_restarts_backoff() {
        let p = ReconnectPolicy::default();
        p.next_delay(); p.next_delay();
        p.reset();
        assert_eq!(p.next_delay(), Duration::from_secs(2));
    }
}
