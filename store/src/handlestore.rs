#![deny(missing_copy_implementations)]

use std::ptr;
use std::ops;
use std::sync::*;
use std::sync::atomic::*;

/// Index into the store to access elements in O(1)
#[derive(PartialEq, Eq, Debug)]
pub struct Index<Data>(*mut Entry<Data>);

impl<Data> Index<Data> {
    pub fn null() -> Index<Data> {
        Index(ptr::null_mut())
    }

    fn new(entry: &mut Box<Entry<Data>>) -> Index<Data> {
        entry.ref_count.fetch_add(1, Ordering::Relaxed);
        Index(entry.as_mut() as *mut Entry<Data>)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn release(&mut self) {
        if !self.is_null() {
            let entry = unsafe { &(*self.0) };
            entry.ref_count.fetch_sub(1, Ordering::Relaxed);
        }
        self.0 = ptr::null_mut();
    }
}

impl<Data> Clone for Index<Data> {
    fn clone(&self) -> Index<Data> {
        if !self.is_null() {
            let entry = unsafe { &(*self.0) };
            entry.ref_count.fetch_add(1, Ordering::Relaxed);
        }
        Index(self.0)
    }
}

impl<Data> Drop for Index<Data> {
    fn drop(&mut self) {
        self.release();
    }
}

/*
/// Unsafe index that does not modify reference counting
pub struct UnsafeIndex<Data>(*mut Entry<Data>);

impl<Data> UnsafeIndex<Data> {
    pub fn null() -> UnsafeIndex<Data> {
        UnsafeIndex(ptr::null_mut())
    }

    pub fn new(idx: &Index<Data>) -> UnsafeIndex<Data> {
        UnsafeIndex(idx.0)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn release(&mut self) {
        self.0 = ptr::null_mut();
    }
}

impl<'a, Data> From<&'a Index<Data>> for UnsafeIndex<Data> {
    #[inline(always)]
    fn from(idx: &'a Index<Data>) -> UnsafeIndex<Data> {
        UnsafeIndex::new(idx)
    }
}
*/

/// An entry in the store.
#[derive(Debug)]
struct Entry<Data> {
    /// Number of active Index (number of references) to this entry
    ref_count: AtomicUsize,
    /// The stored data
    value: Data,
}


/// Resource store.
/// This is a store used for hw resource handling. Id/object can be constructed at any time but
/// update is always performed in a guarded scope.
pub struct Store<Data> {
    resources: Mutex<Vec<Box<Entry<Data>>>>,
    requests: Mutex<Vec<Box<Entry<Data>>>>,
}

impl<Data> Store<Data> {
    pub fn new() -> Store<Data> {
        Store {
            resources: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Creates a new store with memory allocated for at least capacity items
    pub fn new_with_capacity(capacity: usize, request_capacity: usize) -> Store<Data> {
        Store {
            resources: Mutex::new(Vec::with_capacity(capacity)),
            requests: Mutex::new(Vec::with_capacity(request_capacity)),
        }
    }

    /// Adds a new item to the store
    pub fn add(&self, data: Data) -> Index<Data> {
        let mut requests = self.requests.lock().unwrap();
        let mut entry = Box::new(Entry {
            ref_count: AtomicUsize::new(0),
            value: data
        });

        let index = Index::new(&mut entry);
        requests.push(entry);
        index
    }

    /// Returns a guard object that ensures a scope for update.
    /// The stored items cannot be accessed outside this scope and at most one thread may
    /// own an UpdateGuardStore.
    /// # Panic
    /// If multiple threads try to acquire an update a panic is triggered.
    pub fn update<'a>(&'a self) -> UpdateGuardStore<'a, Data> {
        UpdateGuardStore {
            resources: self.resources.try_lock().unwrap(),
            requests: &self.requests,
        }
    }
}

impl<Data> Drop for Store<Data> {
    fn drop(&mut self) {
        let mut resources = self.resources.try_lock().unwrap();
        let mut requests = self.requests.try_lock().unwrap();
        requests.retain(|v| v.ref_count.load(Ordering::Relaxed) > 0);
        resources.retain(|v| v.ref_count.load(Ordering::Relaxed) > 0);

        assert! ( requests.is_empty(), "Leaking resource");
        assert! ( resources.is_empty(), "Leaking requests");
    }
}


/// Helper
pub struct UpdateGuardStore<'a, Data: 'a> {
    resources: MutexGuard<'a, Vec<Box<Entry<Data>>>>,
    requests: &'a Mutex<Vec<Box<Entry<Data>>>>,
}

impl<'a, Data: 'a> UpdateGuardStore<'a, Data> {
    fn drain_unreferenced<F: Fn(&mut Data)>(v: &mut Vec<Box<Entry<Data>>>, f: &F) {
        v.drain_filter(|v| {
            if v.ref_count.load(Ordering::Relaxed) == 0 {
                f(&mut v.value);
                false
            } else {
                true
            }
        });
    }

    /// Release unreferenced resources.
    pub fn drain_unused<F: Fn(&mut Data)>(&mut self, f: F) {
        Self::drain_unreferenced(&mut self.resources, &f);

        let mut requests = self.requests.lock().unwrap();
        Self::drain_unreferenced(&mut requests, &f);
    }

    /// Merge the requests in the "active" items
    pub fn process_requests(&mut self) {
        let mut requests = self.requests.lock().unwrap();
        self.resources.append(requests.as_mut());
    }
}

impl<'a, 'i, Data> ops::Index<&'i Index<Data>> for UpdateGuardStore<'a, Data> {
    type Output = Data;

    fn index(&self, index: &Index<Data>) -> &Self::Output {
        assert! ( !index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &(*index.0) };
        &entry.value
    }
}

impl<'a, 'i, Data> ops::IndexMut<&'i Index<Data>> for UpdateGuardStore<'a, Data> {
    fn index_mut(&mut self, index: &Index<Data>) -> &mut Self::Output {
        assert! ( !index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &mut (*index.0) };
        &mut entry.value
    }
}

/*
impl<'a, 'i, Data> ops::Index<&'i UnsafeIndex<Data>> for UpdateGuardStore<'a, Data> {
    type Output = Data;

    fn index(&self, index: &UnsafeIndex<Data>) -> &Self::Output {
        assert! ( !index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &(*index.0) };
        &entry.value
    }
}

impl<'a, 'i, Data> ops::IndexMut<&'i UnsafeIndex<Data>> for UpdateGuardStore<'a, Data> {
    fn index_mut(&mut self, index: &UnsafeIndex<Data>) -> &mut Self::Output {
        assert! ( !index.is_null(), "Indexing by a null-index is not allowed");
        let entry = unsafe { &mut (*index.0) };
        &mut entry.value
    }
}*/