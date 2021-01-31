use once_cell::sync::OnceCell;
use std::future::Future;
use tokio::sync::Mutex;

pub struct AsyncOnceCell<T> {
    cell: OnceCell<T>,
    lock: Mutex<()>,
}

impl<T> AsyncOnceCell<T> {
    pub const fn new() -> Self {
        Self {
            cell: OnceCell::new(),
            lock: Mutex::const_new(()),
        }
    }

    pub fn with_val(val: T) -> Self {
        let cell = OnceCell::new();
        cell.get_or_init(|| val);

        Self {
            cell,
            lock: Mutex::const_new(()),
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.cell.get()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.cell.get_mut()
    }

    pub async fn get_or_init<F>(&self, f: F) -> &T
    where
        F: Future<Output = T>,
    {
        if let Some(v) = self.cell.get() {
            return v;
        }

        let _ = self.lock.lock().await;

        if let Some(v) = self.cell.get() {
            return v;
        }

        let v = f.await;
        self.cell.get_or_init(|| v)
    }

    pub async fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        if let Some(v) = self.cell.get() {
            return Ok(v);
        }

        let _ = self.lock.lock().await;

        if let Some(v) = self.cell.get() {
            return Ok(v);
        }

        let r = f.await;
        self.cell.get_or_try_init(|| r)
    }

    pub fn into_inner(self) -> Option<T> {
        self.cell.into_inner()
    }

    pub fn swap(&mut self, value: Option<T>) -> Option<T> {
        let old = self.cell.take();

        if let Some(value) = value {
            let _ = self.cell.set(value);
        }

        old
    }

    pub fn take(&mut self) -> Option<T> {
        self.cell.take()
    }
}

impl<T> Default for AsyncOnceCell<T> {
    fn default() -> Self {
        Self {
            cell: OnceCell::new(),
            lock: Mutex::new(()),
        }
    }
}
