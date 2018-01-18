#![deny(missing_copy_implementations)]

#![deny(missing_copy_implementations)]

use std::ptr;
use std::ops;
use std::sync::*;
use std::sync::atomic::*;
use arena::*;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

/// Trait for resource id
pub trait Key: Clone + Send + Eq + Hash + fmt::Debug {}

pub trait Data<K: Key>: From<K> + fmt::Debug {}


/// Reference counted indexing of the store items in O(1).
#[derive(Debug)]
pub struct Index<K: Key, D: Data<K>>(*mut Entry<K, D>);

impl<K: Key, D: Data<K>> Index<K, D> {
    pub fn null() -> Index<K, D> {
        Index(ptr::null_mut())
    }

    fn new(entry: *mut Entry<K, D>) -> Index<K, D> {
        unsafe { &(*entry).ref_count.fetch_add(1, Ordering::Relaxed) };
        Index(entry)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn reset(&mut self) {
        if !self.is_null() {
            unsafe { &(*self.0).ref_count.fetch_sub(1, Ordering::Relaxed) };
        }
        self.0 = ptr::null_mut();
    }
}

impl<K: Key, D: Data<K>> Default for Index<K, D> {
    fn default() -> Index<K, D> {
        Index::null()
    }
}

impl<K: Key, D: Data<K>> Clone for Index<K, D> {
    fn clone(&self) -> Index<K, D> {
        if !self.is_null() {
            unsafe { &(*self.0).ref_count.fetch_add(1, Ordering::Relaxed) };
        }
        Index(self.0)
    }
}

impl<K: Key, D: Data<K>> PartialEq for Index<K, D> {
    fn eq(&self, e: &Self) -> bool {
        self.0 == e.0
    }
}

impl<K: Key, D: Data<K>> Drop for Index<K, D> {
    fn drop(&mut self) {
        self.reset();
    }
}


/// Unsafe indexing of the store items in O(1) without reference count maintenance.
#[derive(Debug)]
pub struct UnsafeIndex<K: Key, D: Data<K>>(*mut Entry<K, D>);

impl<K: Key, D: Data<K>> UnsafeIndex<K, D> {
    pub fn null() -> UnsafeIndex<K, D> {
        UnsafeIndex(ptr::null_mut())
    }

    pub fn from_index(idx: &Index<K, D>) -> UnsafeIndex<K, D> {
        UnsafeIndex(idx.0)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn release(&mut self) {
        self.0 = ptr::null_mut();
    }
}

impl<K: Key, D: Data<K>> Default for UnsafeIndex<K, D> {
    fn default() -> UnsafeIndex<K, D> {
        UnsafeIndex::null()
    }
}

impl<K: Key, D: Data<K>> PartialEq for UnsafeIndex<K, D> {
    fn eq(&self, e: &Self) -> bool {
        self.0 == e.0
    }
}

impl<K: Key, D: Data<K>> Clone for UnsafeIndex<K, D> {
    fn clone(&self) -> UnsafeIndex<K, D> {
        UnsafeIndex(self.0)
    }
}


/// An entry in the store.
#[derive(Debug)]
struct Entry<K: Key, D: Data<K>> {
    /// Number of active Index (number of references) to this entry
    ref_count: AtomicUsize,
    /// The stored data
    value: D,
    phantom: PhantomData<K>,
}


// Store data that requires exclusive lock
struct SharedData<K: Key, D: Data<K>> {
    resources: HashMap<K, *mut Entry<K, D>>,
}

impl<K: Key, D: Data<K>> SharedData<K, D> {
    fn get(&self, k: &K) -> Index<K, D> {
        match self.resources.get(k) {
            Some(&v) => Index::new(v),
            None => Index::null(),
        }
    }
}


// D that requires exclusive lock
struct ExclusiveData<K: Key, D: Data<K>> {
    arena: Arena<Entry<K, D>>,
    requests: HashMap<K, *mut Entry<K, D>>,
}

impl<K: Key, D: Data<K>> ExclusiveData<K, D> {
    fn get(&self, k: &K) -> Index<K, D> {
        match self.requests.get(k) {
            Some(&v) => Index::new(v),
            None => Index::null(),
        }
    }

    /// Adds a new item to the store
    fn get_or_add(&mut self, k: K) -> Index<K, D> {
        let arena = &mut self.arena;
        let entry =
            self.requests.entry(k.clone())
                .or_insert_with( || {
                    let new_entry = arena.allocate(
                        Entry {
                            ref_count: AtomicUsize::new(0),
                            value: D::from(k.clone()),
                            phantom: PhantomData,
                        });
                    new_entry as *mut Entry<K, D>
                });

        Index::new(*entry)
    }
}


/// Resource store.
pub struct HashStore<K: Key, D: Data<K>> {
    shared: RwLock<SharedData<K, D>>,
    exclusive: Mutex<ExclusiveData<K, D>>,
}

unsafe impl<K: Key, D: Data<K>> Send for HashStore<K, D> {}

unsafe impl<K: Key, D: Data<K>> Sync for HashStore<K, D> {}

impl<K: Key, D: Data<K>> HashStore<K, D> {
    pub fn new() -> HashStore<K, D> {
        HashStore {
            shared: RwLock::new(
                SharedData {
                    resources: HashMap::new()
                }),
            exclusive: Mutex::new(
                ExclusiveData {
                    arena: Arena::new(),
                    requests: HashMap::new(),
                }),
        }
    }

    /// Creates a new store with memory allocated for at least capacity items
    pub fn new_with_capacity(_page_size: usize, capacity: usize) -> HashStore<K, D> {
        HashStore {
            shared: RwLock::new(
                SharedData {
                    resources: HashMap::with_capacity(capacity)
                }),
            exclusive: Mutex::new(
                ExclusiveData {
                    arena: Arena::new() /*Arena::_with_capacity(page_size, capacity)*/,
                    requests: HashMap::with_capacity(capacity),
                }),
        }
    }

    /// Returns a read locked access
    pub fn read<'a>(&'a self) -> ReadGuard<'a, K, D> {
        let shared = self.shared.read().unwrap();

        ReadGuard {
            shared: shared,
            exclusive: &self.exclusive,
        }
    }

    /// Returns a write locked access
    pub fn update<'a>(&'a self) -> UpdateGuard<'a, K, D> {
        let shared = self.shared.write().unwrap();
        let exclusive = self.exclusive.lock().unwrap();

        UpdateGuard {
            shared: shared,
            exclusive: exclusive,
        }
    }
}

impl<K: Key, D: Data<K>> Drop for HashStore<K, D> {
    fn drop(&mut self) {
        let shared = &mut *(self.shared.write().unwrap());
        let exclusive = &mut *(self.exclusive.lock().unwrap());
        let arena = &mut exclusive.arena;
        let requests = &mut exclusive.requests;
        let resources = &mut shared.resources;

        resources.retain(|_, &mut v| {
            let v = unsafe { &mut *v };
            assert!(v.ref_count.load(Ordering::Relaxed) == 0, "resource leak");
            arena.deallocate(v);
            false
        });

        requests.retain(|_, &mut v| {
            let v = unsafe { &mut *v };
            assert!(v.ref_count.load(Ordering::Relaxed) == 0, "resource leak");
            arena.deallocate(v);
            false
        });

        assert!(resources.is_empty(), "Leaking resource");
        assert!(requests.is_empty(), "Leaking requests");
        assert!(arena.is_empty(), "Leaking arena, internal store error");
    }
}


/// Guarded read access to a store
pub struct ReadGuard<'a, K: 'a + Key, D: 'a + Data<K>> {
    shared: RwLockReadGuard<'a, SharedData<K, D>>,
    exclusive: &'a Mutex<ExclusiveData<K, D>>,
}

impl<'a, K: 'a + Key, D: 'a + Data<K>> ReadGuard<'a, K, D> {
    pub fn get(&self, k: &K) -> Index<K, D> {
        let index = self.shared.get(k);
        if !index.is_null() {
            return index;
        }

        let exclusive = self.exclusive.lock().unwrap();
        exclusive.get(k)
    }

    pub fn get_or_add(&self, k: K) -> Index<K, D> {
        let index = self.shared.get(&k);
        if !index.is_null() {
            return index;
        }

        let mut exclusive = self.exclusive.lock().unwrap();
        exclusive.get_or_add(k)
    }

    pub fn at(&self, index: &Index<K, D>) -> &D {
        assert!(!index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &(*index.0) };
        &entry.value
    }

    pub unsafe fn unsafe_at(&self, index: &UnsafeIndex<K, D>) -> &D {
        assert!(!index.is_null(), "Indexing by a null-index is not allowed");
        let entry = &(*index.0);
        &entry.value
    }
}

impl<'a, 'i, K: 'a + Key, D: 'a + Data<K>> ops::Index<&'i Index<K, D>> for ReadGuard<'a, K, D> {
    type Output = D;

    fn index(&self, index: &Index<K, D>) -> &Self::Output {
        self.at(index)
    }
}


/// Guarded update access to a store
pub struct UpdateGuard<'a, K: 'a + Key, D: 'a + Data<K>> {
    shared: RwLockWriteGuard<'a, SharedData<K, D>>,
    exclusive: MutexGuard<'a, ExclusiveData<K, D>>,
}

impl<'a, K: 'a + Key, D: 'a + Data<K>> UpdateGuard<'a, K, D> {
    pub fn get(&self, k: &K) -> Index<K, D> {
        let index = self.shared.get(k);
        if !index.is_null() {
            return index;
        }

        self.exclusive.get(k)
    }

    pub fn get_or_add(&mut self, k: K) -> Index<K, D> {
        let index = self.shared.get(&k);
        if !index.is_null() {
            return index;
        }

        self.exclusive.get_or_add(k)
    }

    pub fn is_empty(&self) -> bool {
        self.exclusive.requests.is_empty() && self.shared.resources.is_empty()
    }

    /// Merges the requests into the "active" items
    pub fn finalize_requests(&mut self) {
        // Move all resources into the stored resources
        self.shared.resources.extend(&mut self.exclusive.requests.drain());
    }

    fn retain_impl<F: FnMut(&mut D, bool) -> bool>(arena: &mut Arena<Entry<K, D>>, v: &mut HashMap<K, *mut Entry<K, D>>, filter: &mut F) {
        v.retain(|_k, &mut e| {
            let e = unsafe { &mut *e };
            let is_referenced = e.ref_count.load(Ordering::Relaxed) > 0;
            let is_retain = filter(&mut e.value, is_referenced);
            if !is_referenced & !is_retain {
                arena.deallocate(e);
            }
            is_referenced || is_retain
        });
    }

    /// Retains the referenced elements and the those specified by the predicate.
    /// In other words, remove all unreferenced resources such that f(&mut v) returns false.
    pub fn retain<F: FnMut(&mut D, bool) -> bool>(&mut self, filter: &mut F) {
        let exclusive = &mut *self.exclusive;
        Self::retain_impl(&mut exclusive.arena, &mut self.shared.resources, filter);
        Self::retain_impl(&mut exclusive.arena, &mut exclusive.requests, filter);
    }

    /// Retains the referenced items only.
    /// In other words, remove all unreferenced resources.
    pub fn drain_unused(&mut self) {
        self.retain(&mut |_, _| false)
    }

    pub fn at(&self, index: &Index<K, D>) -> &D {
        assert!(!index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &(*index.0) };
        &entry.value
    }

    pub fn at_mut(&self, index: &Index<K, D>) -> &mut D {
        assert!(!index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &mut (*index.0) };
        &mut entry.value
    }

    pub unsafe fn unsafe_at(&self, index: &UnsafeIndex<K, D>) -> &D {
        assert!(!index.is_null(), "Indexing by a null-index is not allowed");
        let entry = &(*index.0);
        &entry.value
    }

    pub unsafe fn unsafe_at_mut(&self, index: &UnsafeIndex<K, D>) -> &mut D {
        assert!(!index.is_null(), "Indexing by a null-index is not allowed");
        let entry = &mut (*index.0);
        &mut entry.value
    }
}

impl<'a, 'i, K: 'a + Key, D: 'a + Data<K>> ops::Index<&'i Index<K, D>> for UpdateGuard<'a, K, D> {
    type Output = D;

    fn index(&self, index: &Index<K, D>) -> &Self::Output {
        self.at(index)
    }
}

impl<'a, 'i, K: 'a + Key, D: 'a + Data<K>> ops::IndexMut<&'i Index<K, D>> for UpdateGuard<'a, K, D> {
    fn index_mut(&mut self, index: &Index<K, D>) -> &mut Self::Output {
        self.at_mut(index)
    }
}
