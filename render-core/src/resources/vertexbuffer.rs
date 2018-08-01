#![deny(missing_docs)]

use std::mem;
use std::slice;
use std::str::FromStr;
use resources::*;

/// Memory layout, location of a vertex attribute
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum VertexBufferLayoutElement {
    Float32 { stride: usize, offset: usize },
    Float32x2 { stride: usize, offset: usize },
    Float32x3 { stride: usize, offset: usize },
    Float32x4 { stride: usize, offset: usize },
}


/// Trait to define vertex declaration.
pub trait VertexDeclaration: 'static + Clone {
    /// The type used for the attribute indexing.
    type Attribute: 'static + Copy + From<usize> + Into<usize> + FromStr;

    /// Returns an iterator over the possible attribute values.
    fn get_attributes() -> slice::Iter<'static, Self::Attribute>;

    /// Returns the platform dependent vertex attribute description.
    fn get_attribute_layout(index: Self::Attribute) -> VertexBufferLayoutElement;
}


/// Enum to define vertex data.
pub enum VertexData<'a> {
    /// Transient data, a copy is created in the command buffer and no references kept of the source.
    Transient(&'a [u8])
}


/// Trait to define vertex source.
pub trait VertexSource<DECL: VertexDeclaration> {
    /// Returns the reference to the raw vertex data.
    fn to_data<'a>(&self) -> VertexData<'a>;
}


/// VertexSource implementation for arrays. The trait is implemented for array with size up to 32.
/// For larger array, slice can be used:
///
/// let data = [Vertex; 1024];
/// let desc = data.as_ref().to_vertex_data();
///
macro_rules! __impl_array_VertexSource {
    ( $($N: expr) + ) => {
        $(
            /// VertexSource implementation for array.
            impl <DECL: VertexDeclaration + Sized> VertexSource<DECL> for [DECL; $N] {
                fn to_data < 'a > ( & self ) -> VertexData < 'a > {
                    let slice = unsafe { slice::from_raw_parts( self.as_ptr() as * const u8, self.len() * mem::size_of::< DECL > ()) };
                    VertexData::Transient(slice)
                }
            }
        )+
    }
}

__impl_array_VertexSource! {
0  1  2  3  4  5  6  7  8  9
10 11 12 13 14 15 16 17 18 19
20 21 22 23 24 25 26 27 28 29
30 31 32
}


/// VertexSource implementation for slice.
impl<'a, DECL: 'a + VertexDeclaration + Sized> VertexSource<DECL> for &'a [DECL] {
    fn to_data<'b>(&self) -> VertexData<'b>
    {
        let slice = unsafe { slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * mem::size_of::<DECL>()) };
        VertexData::Transient(slice)
    }
}


/// VertexSource implementation for Vec.
impl<DECL: VertexDeclaration + Sized> VertexSource<DECL> for Vec<DECL> {
    fn to_data<'a>(&self) -> VertexData<'a>
    {
        let slice = unsafe { slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * mem::size_of::<DECL>()) };
        VertexData::Transient(slice)
    }
}


/// Trait that defines a vertex buffer with vertex format declaration.
pub trait VertexBuffer<DECL: VertexDeclaration>: Resource {
    /// Reference to an attribute of this vertex buffer used in shader parameters.
    type AttributeRef: 'static + Clone;

    /// Sets the content of the buffer.
    fn set<'a, SRC: VertexSource<DECL>, Q: CommandQueue<Command=Self::Command>>(&self, queue: &Q, source: &SRC);

    /// Gets a referenc to an attribute.
    fn get_attribute(&self, attr: DECL::Attribute) -> Self::AttributeRef;
}
