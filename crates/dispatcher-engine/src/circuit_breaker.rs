use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 熔断器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerSnapshot {
    pub provider_id: String,
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub cooldown_remaining_secs: u64,
}

/// Provider 级别的熔断器
struct ProviderBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    last_failure_time: Option<std::time::Instant>,
    threshold: u32,
    timeout: std::time::Duration,
}

impl ProviderBreaker {
    fn new(threshold: u32, timeout_secs: u64) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            last_failure_time: None,
            threshold,
            timeout: std::time::Duration::from_secs(timeout_secs),
        }
    }

    fn is_open(&self) -> bool {
        matches!(self.state, CircuitBreakerState::Open)
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(std::time::Instant::now());

        if self.failure_count >= self.threshold {
            self.state = CircuitBreakerState::Open;
            tracing::warn!(
                "Circuit breaker OPEN after {} consecutive failures",
                self.failure_count
            );
        }
    }

    fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitBreakerState::Closed;
    }

    fn try_half_open(&mut self) -> bool {
        if self.state != CircuitBreakerState::Open {
            return false;
        }

        if let Some(last) = self.last_failure_time {
            if last.elapsed() >= self.timeout {
                self.state = CircuitBreakerState::HalfOpen;
                tracing::info!("Circuit breaker transitioning to HALF_OPEN");
                return true;
            }
        }
        false
    }
}

/// 熔断器管理器 — 管理所有 provider 的熔断器
pub struct CircuitBreaker {
    breakers: Arc<RwLock<HashMap<String, ProviderBreaker>>>,
    threshold: u32,
    timeout_secs: u64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, timeout_secs: u64) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            threshold,
            timeout_secs,
        }
    }

    /// 检查 provider 是否可用（熔断器是否关闭）
    pub async fn is_available(&self, provider_id: &str) -> bool {
        let breakers = self.breakers.read().await;
        if let Some(breaker) = breakers.get(provider_id) {
            if breaker.is_open() {
                // 尝试 half-open
                drop(breakers);
                let mut write = self.breakers.write().await;
                if let Some(b) = write.get_mut(provider_id) {
                    return b.try_half_open();
                }
            }
            return true;
        }
        true
    }

    /// 记录成功调用
    pub async fn record_success(&self, provider_id: &str) {
        let mut breakers = self.breakers.write().await;
        if let Some(breaker) = breakers.get_mut(provider_id) {
            breaker.record_success();
        }
    }

    /// 记录失败调用
    pub async fn record_failure(&self, provider_id: &str) {
        let mut breakers = self.breakers.write().await;
        let breaker = breakers
            .entry(provider_id.to_string())
            .or_insert_with(|| ProviderBreaker::new(self.threshold, self.timeout_secs));
        breaker.record_failure();
    }

    /// 获取所有被熔断的 provider ID
    pub async fn get_open_providers(&self) -> Vec<String> {
        let mut breakers = self.breakers.write().await;
        breakers
            .iter_mut()
            .filter_map(|(id, breaker)| {
                (breaker.is_open() && !breaker.try_half_open()).then(|| id.clone())
            })
            .collect()
    }

    pub async fn snapshots(&self) -> Vec<CircuitBreakerSnapshot> {
        let mut breakers = self.breakers.write().await;
        let mut snapshots: Vec<_> = breakers
            .iter_mut()
            .map(|(provider_id, breaker)| {
                breaker.try_half_open();
                let cooldown_remaining_secs = if breaker.state == CircuitBreakerState::Open {
                    breaker
                        .last_failure_time
                        .map(|last| breaker.timeout.saturating_sub(last.elapsed()).as_secs())
                        .unwrap_or(0)
                } else {
                    0
                };

                CircuitBreakerSnapshot {
                    provider_id: provider_id.clone(),
                    state: breaker.state,
                    failure_count: breaker.failure_count,
                    cooldown_remaining_secs,
                }
            })
            .collect();
        snapshots.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        snapshots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opens_after_threshold_failures() {
        let cb = CircuitBreaker::new(3, 30);

        cb.record_failure("test-provider").await;
        cb.record_failure("test-provider").await;
        assert!(cb.is_available("test-provider").await);

        cb.record_failure("test-provider").await;
        assert!(!cb.is_available("test-provider").await);
    }

    #[tokio::test]
    async fn success_resets_failures() {
        let cb = CircuitBreaker::new(3, 30);

        cb.record_failure("test-provider").await;
        cb.record_failure("test-provider").await;
        cb.record_success("test-provider").await;

        // 再失败两次不应该触发
        cb.record_failure("test-provider").await;
        cb.record_failure("test-provider").await;
        assert!(cb.is_available("test-provider").await);
    }

    #[tokio::test]
    async fn snapshot_reports_open_provider_and_failure_count() {
        let cb = CircuitBreaker::new(2, 30);

        cb.record_failure("test-provider").await;
        cb.record_failure("test-provider").await;

        let snapshots = cb.snapshots().await;
        let snapshot = snapshots
            .iter()
            .find(|snapshot| snapshot.provider_id == "test-provider")
            .unwrap();

        assert_eq!(snapshot.state, CircuitBreakerState::Open);
        assert_eq!(snapshot.failure_count, 2);
        assert!(snapshot.cooldown_remaining_secs <= 30);
    }
}
