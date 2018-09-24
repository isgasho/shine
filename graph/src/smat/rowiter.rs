use ops::{IntoJoin, Join};
use smat::{DataPosition, DataRange, Entry, MatrixMask, MatrixMaskExt, SMatrix, Store};
use std::mem;
use traits::{IndexExcl, IndexLowerBound};

/// Access a single row in the matrix.
pub struct RowRead<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    //crate row_index: usize,
    crate mask: &'a M,
    crate store: &'a S,
    crate data_range: DataRange,
}

impl<'a, M, S> IndexExcl<usize> for RowRead<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    type Item = &'a S::Item;

    fn index(&mut self, idx: usize) -> Self::Item {
        let DataPosition(pos) = self.mask.find_column_position(idx, self.data_range).unwrap();
        self.store.get(pos)
    }
}

impl<'a, M, S> IndexLowerBound<usize> for RowRead<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    fn lower_bound(&mut self, idx: usize) -> Option<usize> {
        self.mask
            .lower_bound_column_position(idx, self.data_range)
            .map(|(idx, _)| idx)
    }
}

impl<'a, M, S> IntoJoin for RowRead<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    type Store = Self;

    fn into_join(self) -> Join<Self::Store> {
        Join::from_parts(self.mask.get_column_range(self.data_range), self)
    }
}

/// Access a single row in the matrix.
pub struct RowUpdate<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    //crate row_index: usize,
    crate mask: &'a M,
    crate store: &'a mut S,
    crate data_range: DataRange,
}

impl<'a, M, S> IndexExcl<usize> for RowUpdate<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    type Item = &'a mut S::Item;

    fn index(&mut self, idx: usize) -> Self::Item {
        let DataPosition(pos) = self.mask.find_column_position(idx, self.data_range).unwrap();
        unsafe { mem::transmute(self.store.get_mut(pos)) } // GAT
    }
}

impl<'a, M, S> IndexLowerBound<usize> for RowUpdate<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    fn lower_bound(&mut self, idx: usize) -> Option<usize> {
        //perf: we could also increment data range if it's guranted that no "step" occures.
        self.mask
            .lower_bound_column_position(idx, self.data_range)
            .map(|(idx, _)| idx)
    }
}

impl<'a, M, S> IntoJoin for RowUpdate<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    type Store = Self;

    fn into_join(self) -> Join<Self::Store> {
        Join::from_parts(self.mask.get_column_range(self.data_range), self)
    }
}

/// Access a single row in the matrix.
pub struct RowWrite<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    //crate row_index: usize,
    crate row: usize,
    crate mat: &'a mut SMatrix<M, S>,
}

impl<'a, M, S> IndexExcl<usize> for RowWrite<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    type Item = Entry<'a, M, S>;

    fn index(&mut self, idx: usize) -> Self::Item {
        unsafe { mem::transmute(self.mat.get_entry(self.row, idx)) } // GAT
    }
}

impl<'a, M, S> IndexLowerBound<usize> for RowWrite<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    fn lower_bound(&mut self, idx: usize) -> Option<usize> {
        Some(idx)
    }
}

impl<'a, M, S> IntoJoin for RowWrite<'a, M, S>
where
    M: 'a + MatrixMask,
    S: 'a + Store,
{
    type Store = Self;

    fn into_join(self) -> Join<Self::Store> {
        Join::from_parts(0..usize::max_value(), self)
    }
}
