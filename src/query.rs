use crate::archetype::{Archetype, ArchetypeIter, ArchetypeKey};
use crate::component::Component;
use blobvec::BlobVec;
use ecs_macros::impl_queryable_variadic_up_to;
use std::marker::PhantomData;

pub trait Queryable {
    type Key: Copy;
    type IterTuple<'a>;
    type ColumnTuple: Copy;

    fn key() -> Self::Key;
    fn fetch_row<'a>(columns: Self::ColumnTuple, index: usize) -> Self::IterTuple<'a>;
    fn fetch_columns(archetype: &mut Archetype) -> Self::ColumnTuple;
}

impl_queryable_variadic_up_to!(16);

pub struct QueryIter<'e, Q: Queryable> {
    iters: Vec<ArchetypeIter<Q>>,
    current: usize,
    _marker: PhantomData<&'e ()>,
}

impl<'e, Q: Queryable> QueryIter<'e, Q> {
    pub(crate) fn new(iters: Vec<ArchetypeIter<Q>>) -> Self {
        Self {
            iters,
            current: 0,
            _marker: PhantomData,
        }
    }
}

impl<'e, Q: Queryable + 'e> Iterator for QueryIter<'e, Q> {
    type Item = Q::IterTuple<'e>;

    fn next(&mut self) -> Option<Self::Item> {
        let count = self.iters.len();
        while self.current < count {
            let item = unsafe {
                let iter_ptr = (&mut self.iters[self.current]) as *mut ArchetypeIter<Q>;
                (*iter_ptr).next()
            };

            if let Some(item) = item {
                return Some(item);
            }

            self.current += 1;
        }
        None
    }
}
