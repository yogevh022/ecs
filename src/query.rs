use crate::archetype::{Archetype, ArchetypeIter, ArchetypeKey};
use crate::component::Component;
use blobvec::BlobVec;
use ecs_macros::impl_queryable_variadic_up_to;
use std::marker::PhantomData;

pub trait QueryFilter {
    fn include() -> ArchetypeKey {
        ArchetypeKey::EMPTY
    }
    fn exclude() -> ArchetypeKey {
        ArchetypeKey::EMPTY
    }
}

pub struct With<T: Component>(PhantomData<T>);
pub struct Without<T: Component>(PhantomData<T>);

impl QueryFilter for () {}

impl<T: Component> QueryFilter for With<T> {
    fn include() -> ArchetypeKey {
        ArchetypeKey::EMPTY.with_id(T::component_id())
    }
}

impl<T: Component> QueryFilter for Without<T> {
    fn exclude() -> ArchetypeKey {
        ArchetypeKey::EMPTY.with_id(T::component_id())
    }
}

impl<A: QueryFilter, B: QueryFilter> QueryFilter for (A, B) {
    fn include() -> ArchetypeKey {
        A::include() | B::include()
    }
    fn exclude() -> ArchetypeKey {
        A::exclude() | B::exclude()
    }
}

pub trait Queryable {
    type IterTuple<'a>;
    type ColumnTuple: Copy;

    fn key() -> ArchetypeKey;
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
