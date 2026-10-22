use async_cell_lock::{Error, QueueRwLock, Result, deadlock_check};

#[tokio::test]
async fn lock_fails_without_deadlock_check() {
    let lock = QueueRwLock::new((), "lock");
    assert!(matches!(
        lock.queue().await,
        Err(Error::NotDeadlockCheckFuture)
    ));
}

#[deadlock_check]
#[tokio::test]
async fn stacked_with_tokio_test() -> Result<()> {
    let lock = QueueRwLock::new(1, "lock");

    {
        let q = lock.queue().await?;
        *q.write().await? += 1;
    }

    // recursive queue in the same task is detected.
    let _q = lock.queue().await?;
    assert!(lock.queue().await.is_err());
    Ok(())
}

#[deadlock_check("custom_name")]
#[tokio::test(flavor = "multi_thread")]
async fn custom_name_and_unit_return() {
    let lock = QueueRwLock::new((), "lock");
    assert!(lock.read().await.is_ok());
}

#[async_cell_lock::tokio_test]
async fn tokio_test_shorthand() -> Result<()> {
    let lock = QueueRwLock::new((), "lock");
    let _q = lock.queue().await?;
    assert!(lock.queue().await.is_err());
    Ok(())
}

#[async_cell_lock::tokio_test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_test_with_args() {
    let lock = QueueRwLock::new((), "lock");
    assert!(lock.read().await.is_ok());
}

#[deadlock_check]
async fn helper(lock: &QueueRwLock<u32>, add: u32) -> Result<u32> {
    let q = lock.queue().await?;
    let mut w = q.write().await?;
    *w += add;
    Ok(*w)
}

#[tokio::test]
async fn plain_async_fn() -> Result<()> {
    let lock = QueueRwLock::new(1, "lock");
    assert_eq!(helper(&lock, 2).await?, 3);
    assert_eq!(tokio::spawn(async move { helper(&lock, 4).await }).await.unwrap()?, 7);
    Ok(())
}
