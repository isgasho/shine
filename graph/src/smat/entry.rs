use smat::{MatrixMask, SMatrix, Store};
use std::fmt::{self, Debug, Formatter};

/// Entry to a slot in a sparse vector.
pub struct Entry<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    idx: (usize, usize),
    data: Option<*mut S::Item>,
    store: &'a mut SMatrix<M, S>,
}

impl<'a, M, S> Entry<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    crate fn new(store: &mut SMatrix<M, S>, r: usize, c: usize) -> Entry<M, S> {
        Entry {
            idx: (r, c),
            data: store.get_mut(r, c).map(|d| d as *mut _),
            store,
        }
    }

    /// Return the (immutable) non-zero data at the given slot. If data is zero, None is returned.
    pub fn get(&self) -> Option<&S::Item> {
        self.data.map(|d| unsafe { &*d })
    }

    /// Return the (mutable) non-zero data at the given slot. If data is zero, None is returned.
    pub fn get_mut(&mut self) -> Option<&mut S::Item> {
        self.data.map(|d| unsafe { &mut *d })
    }

    // Acquire the mutable non-zero data at the given slot.
    /// If data is zero the provided default value is used.
    pub fn acquire(&mut self, item: S::Item) -> &mut S::Item {
        self.acquire_with(|| item)
    }

    /// Acquire the mutable non-zero data at the given slot.
    /// If data is zero the non-zero value is created using the f function
    pub fn acquire_with<F: FnOnce() -> S::Item>(&mut self, f: F) -> &mut S::Item {
        if self.data.is_none() {
            self.store.add_with(self.idx.0, self.idx.1, f);
            self.data = self.store.get_mut(self.idx.0, self.idx.1).map(|d| d as *mut _);
        }

        self.get_mut().unwrap()
    }

    pub fn remove(&mut self) -> Option<S::Item> {
        match self.data.take() {
            Some(_) => self.store.remove(self.idx.0, self.idx.1),
            None => None,
        }
    }
}

impl<'a, I, M, S> Entry<'a, M, S>
where
    I: Default,
    M: 'a + MatrixMask,
    S: 'a + Store<Item = I>,
{
    pub fn acquire_default(&mut self) -> &mut S::Item {
        self.acquire_with(Default::default)
    }
}

impl<'a, I, M, S> Debug for Entry<'a, M, S>
where
    I: Debug,
    M: 'a + MatrixMask,
    S: 'a + Store<Item = I>,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.get())
    }
}
