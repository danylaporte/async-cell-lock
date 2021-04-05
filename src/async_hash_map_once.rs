use crate::AsyncOnceCell;
use std::{
    borrow::Borrow,
    collections::{hash_map::RandomState, HashMap},
    future::Future,
    hash::{BuildHasher, Hash},
    vec::IntoIter,
};

pub struct AsyncHashMapOnce<K, V, S = RandomState>(
    parking_lot::Mutex<HashMap<K, *mut AsyncOnceCell<V>, S>>,
);

impl<K, V> AsyncHashMapOnce<K, V, RandomState> {
    pub fn new() -> Self {
        AsyncHashMapOnce(Default::default())
    }
}

impl<K, V, S> AsyncHashMapOnce<K, V, S> {
    pub fn clear(&mut self) {
        self.0.get_mut().drain().for_each(|(_, v)| {
            owned(v);
        });
    }

    pub fn drain(&mut self) -> Drain<K, V> {
        Drain(
            self.0
                .get_mut()
                .drain()
                .filter_map(|(k, v)| Some((k, owned(v).take()?)))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
        S: BuildHasher,
    {
        self.0.lock().get(key).and_then(|v| unsafe { &**v }.get())
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
        S: BuildHasher,
    {
        self.0
            .get_mut()
            .get_mut(key)
            .and_then(|v| unsafe { &mut **v }.get_mut())
    }

    fn get_or_create_cell(&self, key: K) -> *mut AsyncOnceCell<V>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        *self
            .0
            .lock()
            .entry(key)
            .or_insert_with(|| raw(AsyncOnceCell::new()))
    }

    pub async fn get_or_init<F>(&self, key: K, init: F) -> &V
    where
        F: Future<Output = V>,
        K: Eq + Hash,
        S: BuildHasher,
    {
        let cell = self.get_or_create_cell(key);
        unsafe { &*cell }.get_or_init(init).await
    }

    pub async fn get_or_try_init<F, E>(&self, key: K, init: F) -> Result<&V, E>
    where
        F: Future<Output = Result<V, E>>,
        K: Eq + Hash,
        S: BuildHasher,
    {
        let cell = self.get_or_create_cell(key);
        unsafe { &*cell }.get_or_try_init(init).await
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        self.0
            .get_mut()
            .insert(key, raw(AsyncOnceCell::with_val(value)))
            .map(owned)
            .and_then(|mut v| v.take())
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
        S: BuildHasher,
    {
        self.0
            .get_mut()
            .remove(key)
            .map(owned)
            .and_then(|mut v| v.take())
    }
}

impl<K, V, S> Default for AsyncHashMapOnce<K, V, S>
where
    S: Default,
{
    fn default() -> Self {
        Self {
            0: Default::default(),
        }
    }
}

impl<K, V, S> Drop for AsyncHashMapOnce<K, V, S> {
    fn drop(&mut self) {
        self.clear();
    }
}

pub struct Drain<K, V>(IntoIter<(K, V)>);

impl<K, V> DoubleEndedIterator for Drain<K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl<K, V> ExactSizeIterator for Drain<K, V> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<K, V> Iterator for Drain<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        self.0.count()
    }
}

fn owned<T>(v: *mut T) -> T {
    unsafe { *Box::from_raw(v) }
}

fn raw<T>(v: T) -> *mut T {
    Box::into_raw(Box::new(v))
}
