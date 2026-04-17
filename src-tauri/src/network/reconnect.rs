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
    /// Número da tentativa atual (começa em 0)
    pub fn attempt(&self) -> u32 {
        self.attempt.load(Ordering::Relaxed)
    }

    /// Retorna (número_da_tentativa, delay) e incrementa o contador atomicamente.
    /// Usar desta forma mantém `attempt()` sincronizado:
    ///   let (attempt, delay) = policy.next_delay_with_attempt();
    ///   info!("Tentativa {}: aguardando {:?}", attempt, delay);
    pub fn next_delay_with_attempt(&self) -> (u32, Duration) {
        let current = self.attempt.fetch_add(1, Ordering::Relaxed);
        let secs = DELAYS_SECS.get(current as usize).copied().unwrap_or(30);
        (current, Duration::from_secs(secs))
    }

    /// Retorna apenas o delay (preservado para compatibilidade interna)
    pub fn next_delay(&self) -> Duration {
        self.next_delay_with_attempt().1
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
