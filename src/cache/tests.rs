//! Unit and integration tests for the multi-level caching system.
//!
//! Unit tests run without external dependencies.
//! Integration tests (marked #[ignore]) require Redis + Postgres.

#[cfg(test)]
mod unit {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    // -------------------------------------------------------------------------
    // Single-flight tests
    // -------------------------------------------------------------------------

    use crate::cache::single_flight::SingleFlight;

    #[tokio::test]
    async fn single_flight_only_one_rebuild_on_concurrent_miss() {
        let sf: Arc<SingleFlight<String>> = SingleFlight::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let sf = sf.clone();
            let counter = counter.clone();
            handles.push(tokio::spawn(async move {
                sf.get_or_rebuild("key", || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        // Simulate rebuild latency
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok("rebuilt".to_string())
                    }
                })
                .await
            }));
        }

        let results: Vec<_> = futures::future::join_all(handles).await;
        for r in &results {
            assert_eq!(r.as_ref().unwrap().as_ref().unwrap(), "rebuilt");
        }

        // Only one rebuild should have been triggered despite 10 concurrent callers.
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn single_flight_propagates_error_to_waiters() {
        let sf: Arc<SingleFlight<String>> = SingleFlight::new();

        let mut handles = Vec::new();
        for _ in 0..5 {
            let sf = sf.clone();
            handles.push(tokio::spawn(async move {
                sf.get_or_rebuild("err_key", || async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Err::<String, String>("rebuild failed".to_string())
                })
                .await
            }));
        }

        let results: Vec<_> = futures::future::join_all(handles).await;
        for r in results {
            assert!(r.unwrap().is_err());
        }
    }

    // -------------------------------------------------------------------------
    // Warming state tests
    // ----------------------------------