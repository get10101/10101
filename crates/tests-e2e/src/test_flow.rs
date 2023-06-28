/// Waits until the specified condition is met
#[macro_export]
macro_rules! wait_until {
    ($expr:expr) => {
        // Waiting time for the time on the watch channel before returning error
        let next_wait_time: std::time::Duration = std::time::Duration::from_secs(60);

        let result = tokio::time::timeout(next_wait_time, async {
            let mut wait_time = std::time::Duration::from_millis(10);
            loop {
                if $expr {
                    break;
                }
                tokio::time::sleep(wait_time).await;
                wait_time *= 2; // Increase wait time exponentially
            }
        })
        .await;
        match result {
            Ok(_) => {
                tracing::debug!("Expression satisfied: {}", quote::quote!($expr));
            }
            Err(_) => {
                panic!(
                    "Expression timed out after {}s. Expression: {}",
                    next_wait_time.as_secs(),
                    quote::quote!($expr)
                );
            }
        }
    };
}
