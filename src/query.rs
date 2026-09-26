use crate::archetype::{Archetype, ArchetypeId, ArchetypeIter, ArchetypeKey};
use crate::component::{Component, Components};
use crate::ecs::Ecs;
use crate::system::SysParam;
use blobvec::BlobVec;
use ecs_macros::{impl_component_group_variadic_up_to, impl_queryable_variadic_up_to};
use std::marker::PhantomData;

impl_queryable_variadic_up_to!(16);
impl_component_group_variadic_up_to!(16);

pub trait QueryFilter {
    fn include(_components: &Components) -> ArchetypeKey {
        ArchetypeKey::EMPTY
    }
    fn exclude(_components: &Components) -> ArchetypeKey {
        ArchetypeKey::EMPTY
    }
}

pub trait ComponentGroup {
    fn key(components: &Components) -> ArchetypeKey;
}

impl<T: Component> ComponentGroup for T {
    fn key(components: &Components) -> ArchetypeKey {
        ArchetypeKey::EMPTY.with_id(components.id_of::<T>())
    }
}

pub struct With<T: ComponentGroup>(PhantomData<T>);
pub struct Without<T: ComponentGroup>(PhantomData<T>);

impl QueryFilter for () {}

impl<T: ComponentGroup> QueryFilter for With<T> {
    fn include(components: &Components) -> ArchetypeKey {
        T::key(components)
    }
}

impl<T: ComponentGroup> QueryFilter for Without<T> {
    fn exclude(components: &Components) -> ArchetypeKey {
        T::key(components)
    }
}

impl<A: QueryFilter, B: QueryFilter> QueryFilter for (A, B) {
    fn include(components: &Components) -> ArchetypeKey {
        A::include(components) | B::include(components)
    }
    fn exclude(components: &Components) -> ArchetypeKey {
        A::exclude(components) | B::exclude(components)
    }
}

pub trait Queryable {
    type IterTuple<'a>;
    type ColumnTuple: Copy;
    type ComponentIds: Copy;

    fn component_ids(components: &Components) -> Self::ComponentIds;
    fn key(component_ids: Self::ComponentIds) -> ArchetypeKey;
    fn fetch_row<'a>(columns: Self::ColumnTuple, index: usize) -> Self::IterTuple<'a>;
    fn fetch_columns(archetype: &mut Archetype, component_ids: Self::ComponentIds) -> Self::ColumnTuple;
}

pub struct Query<'e, Q: Queryable, F: QueryFilter = ()> {
    iters: Vec<ArchetypeIter<Q>>,
    current: usize,
    _marker: PhantomData<(&'e (), F)>,
}

impl<'e, Q: Queryable, F: QueryFilter> Query<'e, Q, F> {
    pub(crate) fn new(iters: Vec<ArchetypeIter<Q>>) -> Self {
        Self {
            iters,
            current: 0,
            _marker: PhantomData,
        }
    }
}

impl<'e, Q: Queryable + 'e, F: QueryFilter> Iterator for Query<'e, Q, F> {
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

pub struct QueryState<Q: Queryable> {
    pub with: ArchetypeKey,
    pub without: ArchetypeKey,
    pub component_ids: Q::ComponentIds,
    matched: Vec<ArchetypeId>,
    seen: usize,
}

impl<Q: Queryable> QueryState<Q> {
    pub(crate) fn new<F: QueryFilter>(components: &Components) -> Self {
        let component_ids = Q::component_ids(components);
        Self {
            with: Q::key(component_ids) | F::include(components),
            without: F::exclude(components),
            component_ids,
            matched: Vec::new(),
            seen: 0,
        }
    }

    pub(crate) fn update_matched(&mut self, keys: &[ArchetypeKey]) {
        for id in self.seen..keys.len() {
            let key = &keys[id];
            if key.matches(&self.with, &self.without) {
                self.matched.push(id);
            }
        }
        self.seen = keys.len();
    }

    pub(crate) fn matched(&self) -> &[ArchetypeId] {
        &self.matched
    }
}

impl<Q: Queryable + 'static, F: QueryFilter> SysParam for Query<'_, Q, F> {
    type Item<'a> = Query<'a, Q, F>;
    type State = QueryState<Q>;
    fn init(ecs: &mut Ecs) -> Self::State {
        QueryState::new::<F>(&ecs.components)
    }
    fn fetch<'a>(ecs: *mut Ecs, state: &mut Self::State) -> Self::Item<'a> {
        unsafe { (*ecs).query_state::<Q, F>(state) }
    }
}
