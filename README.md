# async-cell-lock

Async-aware locks and lazily initialized cells for [tokio] with **runtime deadlock detection**.

Every lock acquired through this crate is tracked per task. Instead of hanging forever, an
operation that would deadlock returns an [`Error`](#errors):

- re-locking a lock already held by the same task returns `Error::RecursiveLock`,
- waiting on a lock held by a task that is itself waiting on one of our locks returns
  `Error::DeadlockDetected`.

## Contents

- [Deadlock-check scope](#deadlock-check-scope)
- [Macros](#macros)
  - [`#[tokio_test]`](#tokio_test)
  - [`#[deadlock_check]`](#deadlock_check)
- [Locks](#locks)
  - [`QueueRwLock`](#queuerwlock)
  - [`sync` module](#sync-module)
- [Cells](#cells)
- [Errors](#errors)
- [Cargo features](#cargo-features)

## Deadlock-check scope

Lock tracking is stored in tokio task-locals, so every future that acquires a tracked lock must
run inside a deadlock-check scope. Outside of one, lock operations return
`Error::NotDeadlockCheckFuture`.

There are three ways to create a scope:

```rust
use async_cell_lock::{QueueRwLock, Result, spawn_with_deadlock_check, with_deadlock_check};
use std::sync::LazyLock;

static COUNTER: LazyLock<QueueRwLock<u32>> = LazyLock::new(|| QueueRwLock::new(0, "counter"));

async fn increment() -> Result<u32> {
    let queue = COUNTER.queue().await?;
    let mut w = queue.write().await?;
    *w += 1;
    Ok(*w)
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Wrap a future.
    with_deadlock_check(increment(), "increment").await?;

    // 2. Spawn a wrapped future on the tokio runtime.
    spawn_with_deadlock_check(increment(), "spawned").await.unwrap()?;

    // 3. Use the `#[deadlock_check]` / `#[tokio_test]` attributes (see below).
    Ok(())
}
```

The task name is used in error reports and telemetry.

## Macros

The macros remove the `with_deadlock_check(async move { ... }, "name").await` boilerplate.

### `tokio_test`

`#[async_cell_lock::tokio_test]` replaces `#[tokio::test]` and runs the test body inside a
deadlock-check scope named after the test function. It is exactly equivalent to:

```rust,ignore
#[async_cell_lock::deadlock_check]
#[tokio::test]
```

```rust
use async_cell_lock::{QueueRwLock, Result};

#[async_cell_lock::tokio_test]
async fn my_test() -> Result<()> {
    let lock = QueueRwLock::new(1, "lock");
    *lock.queue().await?.write().await? += 1;
    assert_eq!(*lock.read().await?, 2);
    Ok(())
}
```

All arguments are forwarded to `#[tokio::test]`:

```rust
#[async_cell_lock::tokio_test(flavor = "multi_thread", worker_threads = 2)]
async fn my_multi_thread_test() {
    // ...
}
```

It requires `tokio` with the `macros` feature (and `rt-multi-thread` for the `multi_thread`
flavor) in your `[dev-dependencies]`, just like `#[tokio::test]`.

### `deadlock_check`

`#[async_cell_lock::deadlock_check]` can be put on any `async fn`. It rewrites the body into:

```rust,ignore
async_cell_lock::with_deadlock_check(async move { /* original body */ }, "fn_name").await
```

Arguments, `?`, early `return` and the return type keep working unchanged.

```rust
use async_cell_lock::{QueueRwLock, Result, deadlock_check};

// Any async fn: the returned future is self-contained and can be given to `tokio::spawn`.
#[deadlock_check]
async fn add(lock: &'static QueueRwLock<u32>, value: u32) -> Result<u32> {
    let mut w = lock.queue().await?.write().await?;
    *w += value;
    Ok(*w)
}

// Custom task name: any expression implementing `Into<String>`.
#[deadlock_check("job::nightly")]
async fn nightly_job() {}
```

It can be stacked with runtime attributes such as `#[tokio::test]` or `#[actix_rt::test]`,
but it **must be placed above them**. Attributes expand top-down and runtime attributes turn
the function into a regular `fn`; placing `#[deadlock_check]` below produces a compile error.

```rust
use async_cell_lock::Result;

#[async_cell_lock::deadlock_check("custom_test_name")]
#[tokio::test(flavor = "current_thread")]
async fn my_test() -> Result<()> {
    Ok(())
}
```

Note that a scope is per task: nesting `#[deadlock_check]` functions creates a new, independent
scope for the inner call, so lock tracking does not span across them.

## Locks

All tracked locks take a `&'static str` name used in errors and telemetry. The `sync` lock
constructors are `const` so they can be used in `static`s; `QueueRwLock::new` is not (use a
`OnceLock`/`LazyLock` for a static `QueueRwLock`).

### `QueueRwLock`

An async read/write lock with an extra **queue** step for writers:

1. `queue()` acquires an exclusive ticket. Only one task can hold the queue, but readers are
   still allowed. Do the expensive preparation work while holding it.
2. `QueueRwLockQueueGuard::write()` then waits for readers to drain and grants exclusive
   write access, releasing the queue.
3. A write guard can be turned back into a read guard (`read()`) or a queue guard (`queue()`).

This keeps the exclusive write section as short as possible.

```rust
use async_cell_lock::{QueueRwLock, Result, with_deadlock_check};

#[tokio::main]
async fn main() -> Result<()> {
    with_deadlock_check(
        async {
            let lock = QueueRwLock::new(vec![1, 2], "numbers");

            let queue = lock.queue().await?;
            let new_value = 3; // prepare while readers are still allowed
            let mut w = queue.write().await?;
            w.push(new_value);

            let r = w.read().await?; // downgrade
            assert_eq!(*r, vec![1, 2, 3]);
            Ok(())
        },
        "main",
    )
    .await
}
```

`try_queue()` returns `None` instead of waiting when another task holds the queue.

### `sync` module

Deadlock-checked equivalents of the common lock types:

| Type                          | Backed by                | Notes                                                                                    |
| ----------------------------- | ------------------------ | ---------------------------------------------------------------------------------------- |
| `sync::async_mutex::Mutex`    | `tokio::sync::Mutex`     | `lock().await`                                                                           |
| `sync::async_rwlock::RwLock`  | `tokio::sync::RwLock`    | `read().await`, `write().await`                                                          |
| `sync::mutex::Mutex`          | `parking_lot::Mutex`     | Sync lock that may be held across `.await`.                                              |
| `sync::rwlock::RwLock`        | `parking_lot::RwLock`    | Sync lock that may be held across `.await`.                                              |

When the sync locks are contended inside a tokio runtime, they wait at most 50 ms and then
return `Error::SyncLockTimeout` instead of blocking the runtime thread indefinitely.

## Cells

These types do not need a deadlock-check scope.

- `AsyncOnceCell<T>`: a cell initialized once by an async (`get_or_init`, `get_or_try_init`)
  or sync (`get_or_init_sync`, `get_or_try_init_sync`) initializer. Reading an initialized
  value is a single atomic load, and concurrent async initializers run the initializer only
  once. Inside a deadlock-check scope, a recursive or deadlocking initialization falls back to
  an unsynchronized initialization (first value stored wins) instead of hanging.
- `AsyncLoadRwLock<T>`: a `tokio::sync::RwLock<Option<T>>` that lazily loads its value on the
  first `read_or_init` / `write_or_init` (or the `try_` variants). Write guards can be
  downgraded to read guards.

```rust
use async_cell_lock::AsyncOnceCell;

static CONFIG: AsyncOnceCell<String> = AsyncOnceCell::new();

#[tokio::main]
async fn main() {
    let v = CONFIG.get_or_init(async { "loaded".to_string() }).await;
    assert_eq!(v, "loaded");
}
```

## Errors

| Variant                  | Cause                                                                          |
| ------------------------ | ------------------------------------------------------------------------------ |
| `RecursiveLock`          | The current task already holds this lock.                                      |
| `DeadlockDetected`       | A task holding this lock is waiting on a lock held by the current task.        |
| `NotDeadlockCheckFuture` | The lock was used outside of a deadlock-check scope.                           |
| `SyncLockTimeout`        | A `sync` lock could not be acquired within 50 ms inside an async runtime.      |

`Result<T>` is an alias for `std::result::Result<T, Error>`.

## Cargo features

| Feature        | Description                                                                                                     |
| -------------- | --------------------------------------------------------------------------------------------------------------- |
| `telemetry`    | Emits [`metrics`] (lock await/held gauges and counters, per task) and [`tracing`] events, and adds `warn_lock_held()`. |
| `actix_web_04` | Adds the `DeadlockDetector` middleware for actix-web 4, wrapping each request in a scope named `"{METHOD} {route}"`. |

```rust,ignore
use actix_web::App;
use async_cell_lock::DeadlockDetector;

let app = App::new().wrap(DeadlockDetector);
```

[tokio]: https://tokio.rs
[`metrics`]: https://docs.rs/metrics
[`tracing`]: https://docs.rs/tracing
