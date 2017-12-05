#![cfg(WIP)]
#![deny(missing_copy_implementations)]

use std::ptr;
use std::ops;
use std::sync::*;
use std::sync::atomic::*;

/// Index into the store to access elements in O(1)
#[derive(PartialEq, Eq, Debug)]
pub struct Index<Data>(*const Entry<Data>);

impl<Data> Index<Data> {
    pub fn null() -> Index<Data> {
        Index(ptr::null())
    }

    fn new(entry: &Box<Entry<Data>>) -> Index<Data> {
        entry.ref_count.fetch_add(1, Ordering::Relaxed);
        Index(entry.as_ref() as *const Entry<Data>)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn release(&mut self) {
        *self = Self::null();
    }
}

impl<Data> Clone for Index<Data> {
    fn clone(&self) -> Index<Data> {
        if !self.0.is_null() {
            let entry = unsafe { &(*self.0) };
            entry.ref_count.fetch_add(1, Ordering::Relaxed);
        }
        Index(self.0)
    }
}

impl<Data> Drop for Index<Data> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let entry = unsafe { &*self.0 };
            entry.ref_count.fetch_sub(1, Ordering::Relaxed);
        }
    }
}


/// An entry in the store.
#[derive(Debug)]
struct Entry<Data> {
    /// Number of active Index (number of references) to this entry
    ref_count: AtomicUsize,
    /// The stored data
    value: Data,
}


/// Stores requests for the missing resources.
struct RequestHandler<Data> {
    factory: Box<Fn() -> Data>,
    requests: Vec<Option<Box<Entry<Data>>>>,
}

impl<Data> RequestHandler<Data> {
    fn process_requests(&mut self, resources: &mut Vec<Box<Entry<Data>>>) {
        let factory = &mut self.factory;
        self.requests.retain(|entry| {
            let retain = entry.as_mut().map_or(false, |entry| {
                if entry.meta.is_none() {
                    false
                } else if let Some(value) = factory.create(id, entry.meta.as_mut().unwrap()) {
                    entry.value = value;
                    entry.meta = None;
                    false
                } else {
                    true
                }
            });

            if !retain {
                resources.insert(id.clone(), entry.take().unwrap());
            }
            retain
        });
    }
}


/// Resource store. Resources are constants and can be created/requested through the store only.
pub struct Store<Data> {
    request_handler: Mutex<RequestHandler<Data>>,
    resources: RwLock<Vec<Box<Entry<Data>>>>,
}

impl<Data> Store<Data> {
    /// Creates a new store with the given factory.
    pub fn new(factory: Fn() -> Data) -> Store<Data> {
        Store {
            request_handler: Mutex::new(RequestHandler {
                factory: Box::new(factory),
                requests: Vec::new()
            }),
            resources: RwLock::new(Vec::new()),
        }
    }

    /// Process the requests.
    pub fn update(&self) {
        let mut resources = self.resources.try_write().unwrap();
        let mut request_handler = self.request_handler.lock().unwrap();

        request_handler.process_requests(&mut resources);
    }

    /// Removes unreferenced resources.
    pub fn drain_unused(&self) {
        let mut resources = self.resources.try_write().unwrap();
        let mut request_handler = self.request_handler.lock().unwrap();

        request_handler.requests.retain(|v| {
            let count = v.as_ref().unwrap().ref_count.load(Ordering::Relaxed);
            count != 0
        });

        resources.retain(|v| {
            let count = v.ref_count.load(Ordering::Relaxed);
            count != 0
        });
    }

    /// Returns if there are any pending requests
    pub fn has_request(&self) -> bool {
        let _resources = self.resources.try_write().unwrap();
        let request_handler = self.request_handler.lock().unwrap();

        !request_handler.requests.is_empty()
    }

    /// Returns if there are any elements in the store
    pub fn is_empty(&self) -> bool {
        let resources = self.resources.try_write().unwrap();
        let request_handler = self.request_handler.lock().unwrap();

        request_handler.requests.is_empty() && resources.is_empty()
    }

    /// Returns a guard object that ensures, no resources are updated during its scope.
    pub fn read<'a>(&'a self) -> ReadGuardStore<F> {
        ReadGuardStore {
            resources: self.resources.read().unwrap(),
            request_handler: &self.request_handler,
        }
    }
}

impl<Data> Drop for Store<Data> {
    fn drop(&mut self) {
        self.drain_unused();

        assert!( resources.is_empty(), "Leaking loaded resource");
        assert!( self.request_handler.lock().unwrap().requests.is_empty(), "Leaking requested resource");
    }
}

/// Helper
pub struct ReadGuardStore<'a, Data: 'a> {
    resources: RwLockReadGuard<'a, Vec<Box<Entry<Data>>>>,
    request_handler: &'a Mutex<RequestHandler<Data>>,
}


impl<'a, Data: 'a> ReadGuardStore<'a, Data> {
    fn request_unchecked(&self, id: &F::Key) -> Index<F> {
        let mut request_handler = self.request_handler.lock().unwrap();

        if let Some(request) = request_handler.requests.get(id) {
            // resource already found in the requests container
            return Index::new(request.as_ref().unwrap());
        }

        // resource not found, create a temporary now
        let value = request_handler.factory.request(id);
        let request = Box::new(Entry {
            ref_count: AtomicUsize::new(0),
            value: value.0,
            meta: value.1,
        });
        let index = Index::new(&request);
        request_handler.requests.insert(id.clone(), Some(request));
        index
    }

    /// Returns if there are any pending requests
    pub fn has_request(&self) -> bool {
        let request_handler = self.request_handler.lock().unwrap();
        !request_handler.requests.is_empty()
    }

    /// Gets a loaded resource. If resource is not found, a none reference is returned
    pub fn get(&self, id: &F::Key) -> Index<F> {
        if let Some(request) = self.resources.get(id) {
            // resource already found in the requests container
            return Index::new(request);
        }

        let request_handler = self.request_handler.lock().unwrap();
        if let Some(request) = request_handler.requests.get(id) {
            // resource already found in the requests container
            return Index::new(request.as_ref().unwrap());
        }
        Index::null()
    }

    /// Requests a resource to be loaded without taking a reference to it.
    pub fn request(&self, id: &F::Key) {
        if !self.resources.contains_key(id) {
            self.request_unchecked(id);
        }
    }

    /// Gets a resource by the given id. If the resource is not loaded, reference to pending resource is
    /// returned and the missing is is enqued in the request list.
    pub fn get_or_request(&self, id: &F::Key) -> Index<F> {
        let resource = self.get(id);
        if resource.is_null() {
            self.request_unchecked(id)
        } else {
            resource
        }
    }

    pub fn is_pending(&self, index: &Index<F>) -> bool {
        assert! ( !index.is_null(), "Indexing store by a null-index is not allowed");
        let entry = unsafe { &(*index.0) };
        entry.meta.is_some()
    }

    pub fn is_ready(&self, index: &Index<F>) -> bool {
        !self.is_pending(index)
    }
}

impl<'a, 'i, F: Factory> ops::Index<&'i Index<F>> for ReadGuardStore<'a, F> {
    type Output = F::Data;

    fn index(&self, index: &Index<F>) -> &Self::Output {
        assert! ( !index.is_null(), "Indexing store by a null-index is not allowed");
        let entry = unsafe { &(*index.0) };
        &entry.value
    }
}
