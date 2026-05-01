//! Circuit breaker pattern for webhook delivery.
//!
//! Prevents cascading failures by stopping delivery attempts after
//! consecutive failures, with automatic recovery testing.

use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through.
    Closed,
    /// Failing fast - requests rejected immediately.
    Open,
    /// Testing if service has recovered.
    HalfOpen,
}

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit.
    pub failure_threshold: u32,
    /// Duration to wait before attempting recovery (half-open).
    pub reset_timeout: Duration,
    /// Successes required in half-open state to close circuit.
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 3,
        }
    }
}

/// Circuit breaker for protecting webhook endpoints.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    failures: AtomicU32,
    successes: AtomicU32,
    last_failure: RwLock<Option<Instant>>,
    state: RwLock<CircuitState>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            failures: AtomicU32::new(0),
            successes: AtomicU32::new(0),
            last_failure: RwLock::new(None),
            state: RwLock::new(CircuitState::Closed),
        }
    }

    /// Get the current circuit state.
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Execute a function with circuit breaker protection.
    ///
    /// # Returns
    ///
    /// - `Ok(result)` if the function succeeds
    /// - `Err(CircuitBreakerError::Open)` if circuit is open
    /// - `Err(CircuitBreakerError::Inner(e))` if the function fails
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        // Check if we should transition from Open to HalfOpen
        self.try_transition_to_half_open().await;

        // Check current state
        let state = *self.state.read().await;
        match state {
            CircuitState::Open => {
                return Err(CircuitBreakerError::Open);
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // Proceed with the call
            }
        }

        // Execute the function
        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(CircuitBreakerError::Inner(e))
            }
        }
    }

    /// Record a successful call.
    async fn on_success(&self) {
        match *self.state.read().await {
            CircuitState::HalfOpen => {
                let successes = self.successes.fetch_add(1, Ordering::SeqCst) + 1;
                if successes >= self.config.success_threshold {
                    // Transition to Closed
                    let mut state = self.state.write().await;
                    if *state == CircuitState::HalfOpen {
                        *state = CircuitState::Closed;
                        self.failures.store(0, Ordering::SeqCst);
                        self.successes.store(0, Ordering::SeqCst);
                        tracing::info!("Circuit breaker closed after {} successes", successes);
                    }
                }
            }
            CircuitState::Closed => {
                // Reset failures on success in closed state
                self.failures.store(0, Ordering::SeqCst);
            }
            CircuitState::Open => {
                // Shouldn't happen, but reset anyway
                self.failures.store(0, Ordering::SeqCst);
            }
        }
    }

    /// Record a failed call.
    async fn on_failure(&self) {
        let failures = self.failures.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure.write().await = Some(Instant::now());

        match *self.state.read().await {
            CircuitState::Closed => {
                if failures >= self.config.failure_threshold {
                    // Transition to Open
                    let mut state = self.state.write().await;
                    if *state == CircuitState::Closed {
                        *state = CircuitState::Open;
                        tracing::warn!(
                            "Circuit breaker opened after {} consecutive failures",
                            failures
                        );
                    }
                }
            }
            CircuitState::HalfOpen => {
                // Transition back to Open on any failure in half-open
                let mut state = self.state.write().await;
                if *state == CircuitState::HalfOpen {
                    *state = CircuitState::Open;
                    self.successes.store(0, Ordering::SeqCst);
                    tracing::warn!("Circuit breaker re-opened due to failure in half-open state");
                }
            }
            CircuitState::Open => {
                // Already open, just update last failure time
            }
        }
    }

    /// Try to transition from Open to HalfOpen if reset timeout has passed.
    async fn try_transition_to_half_open(&self) {
        let state = *self.state.read().await;
        if state != CircuitState::Open {
            return;
        }

        let last_failure = *self.last_failure.read().await;
        if let Some(last) = last_failure {
            if last.elapsed() >= self.config.reset_timeout {
                let mut state = self.state.write().await;
                if *state == CircuitState::Open {
                    *state = CircuitState::HalfOpen;
                    self.successes.store(0, Ordering::SeqCst);
                    tracing::info!("Circuit breaker entering half-open state for recovery test");
                }
            }
        }
    }
}

/// Errors from circuit breaker operations.
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open - request rejected.
    Open,
    /// Inner function failed.
    Inner(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::Open => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::Inner(e) => write!(f, "Inner error: {}", e),
        }
    }
}

impl<E: std::error::Error> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::Inner(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn circuit_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new(config);

        // 3 failures should open the circuit
        for _ in 0..3 {
            let _ = cb.call(|| async { Err::<(), ()>(()) }).await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn circuit_rejects_when_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // 1 failure opens circuit
        let _ = cb.call(|| async { Err::<(), ()>(()) }).await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Next call should be rejected
        let result = cb.call(|| async { Ok::<(), ()>(()) }).await;
        assert!(matches!(result, Err(CircuitBreakerError::Open)));
    }

    #[tokio::test]
    async fn successes_reset_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // 2 failures
        for _ in 0..2 {
            let _ = cb.call(|| async { Err::<(), ()>(()) }).await;
        }
        assert_eq!(cb.state().await, CircuitState::Closed);

        // 1 success resets counter
        let _ = cb.call(|| async { Ok::<(), ()>(()) }).await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        // 2 more failures shouldn't open (count was reset)
        for _ in 0..2 {
            let _ = cb.call(|| async { Err::<(), ()>(()) }).await;
        }
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
