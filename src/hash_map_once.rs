use parking_lot::Mutex;
use std::{
    borrow::Borrow,
    collections::{
        hash_map::{Entry, RandomState},
        HashMap,
    },
    hash::{BuildHasher, Hash},
    vec::IntoIter,
};

pub struct HashMapOnce<K, V, S = RandomState>(Mutex<HashMap<K, *mut V, S>>);

impl<K, V> HashMapOnce<K, V, RandomState> {
    pub fn new() -> Self {
        HashMapOnce(Default::default())
    }
}

impl<K, V, S> HashMapOnce<K, V, S> {
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
                .map(|(k, v)| (k, owned(v)))
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
        self.0.lock().get(key).map(|v| unsafe { &**v })
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
        S: BuildHasher,
    {
        self.0.get_mut().get_mut(key).map(|v| unsafe { &mut **v })
    }

    pub fn get_or_init<F>(&self, key: K, init: F) -> &V
    where
        F: FnOnce() -> V,
        K: Eq + Hash,
        S: BuildHasher,
    {
        let mut map = self.0.lock();
        let v = map.entry(key).or_insert_with(|| raw(init()));

        unsafe { &**v }
    }

    pub fn get_or_try_init<F, E>(&self, key: K, init: F) -> Result<&V, E>
    where
        F: FnOnce() -> Result<V, E>,
        K: Eq + Hash,
        S: BuildHasher,
    {
        let mut map = self.0.lock();

        Ok(match map.entry(key) {
            Entry::Occupied(o) => unsafe { &**o.get() },
            Entry::Vacant(v) => unsafe { &**v.insert(raw(init()?)) },
        })
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        self.0.get_mut().insert(key, raw(value)).map(owned)
    }

    pub fn len(&self) -> usize {
        self.0.lock().len()
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
        S: BuildHasher,
    {
        self.0.get_mut().remove(key).map(|v| owned(v))
    }

    pub fn set(&self, key: K, value: V) -> Result<(), V>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        match self.0.lock().entry(key) {
            Entry::Occupied(_) => Err(value),
            Entry::Vacant(v) => {
                v.insert(Box::into_raw(Box::new(value)));
                Ok(())
            }
        }
    }
}

impl<K, V, S> Default for HashMapOnce<K, V, S>
where
    S: Default,
{
    fn default() -> Self {
        Self {
            0: Default::default(),
        }
    }
}

impl<K, V, S> Drop for HashMapOnce<K, V, S> {
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
