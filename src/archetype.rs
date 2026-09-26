use crate::component;
use crate::component::{
    ARCHETYPE_KEY_WORD_BITS, ARCHETYPE_KEY_WORDS, Component, ComponentId, Components,
};
use crate::world::ComponentBox;
use crate::entity::Entity;
use crate::query::{Query, QueryFilter, QueryState, Queryable};
use blobvec::BlobVec;
use rustc_hash::FxHashMap;
use std::any::type_name;
use std::fmt::Debug;
use std::ops::BitOr;

pub type ArchetypeId = usize;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ArchetypeKey(pub(crate) [usize; ARCHETYPE_KEY_WORDS]);
impl ArchetypeKey {
    pub const EMPTY: ArchetypeKey = ArchetypeKey([0; ARCHETYPE_KEY_WORDS]);

    #[inline]
    fn bit_index(comp_id: ComponentId) -> (usize, usize) {
        let component_bit = comp_id - 1;
        (
            component_bit / ARCHETYPE_KEY_WORD_BITS,
            component_bit % ARCHETYPE_KEY_WORD_BITS,
        )
    }

    #[inline]
    pub fn with_id(mut self, comp_id: ComponentId) -> Self {
        let (word, bit) = Self::bit_index(comp_id);
        self.0[word] |= 1 << bit;
        self
    }

    #[inline]
    pub fn without_id(mut self, comp_id: ComponentId) -> Self {
        let (word, bit) = Self::bit_index(comp_id);
        self.0[word] &= !(1 << bit);
        self
    }

    pub fn contains(&self, other: &Self) -> bool {
        for (word, other_word) in self.0.iter().zip(other.0.iter()) {
            if word & other_word != *other_word {
                return false;
            }
        }
        true
    }

    pub fn disjoint(&self, other: &Self) -> bool {
        for (word, other_word) in self.0.iter().zip(other.0.iter()) {
            if word & other_word != 0 {
                return false;
            }
        }
        true
    }

    #[inline]
    pub(crate) fn matches(&self, with: &Self, without: &Self) -> bool {
        self.contains(with) && self.disjoint(without)
    }

    pub fn component_count(&self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
    }
}

impl BitOr for ArchetypeKey {
    type Output = Self;

    #[inline]
    fn bitor(self, other: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] | other.0[i]))
    }
}

impl Debug for ArchetypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArchKey<")?;
        for (i, word) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", word)?;
        }
        write!(f, ">")
    }
}

pub struct Archetype {
    components: Vec<(ComponentId, BlobVec)>,
    entities: Vec<Entity>,
    row_capacity: usize,
}

impl Archetype {
    /// First row slab for a new archetype. Large enough that a short spawn
    /// burst does not grow immediately, small enough that a one-off archetype
    /// does not hold a big empty allocation.
    const INITIAL_ROWS: usize = 16;

    pub(crate) fn new() -> Self {
        Self {
            components: Vec::new(),
            entities: Vec::new(),
            row_capacity: 0,
        }
    }

    /// Creates a new empty archetype with the same component types as the given archetype.
    pub(crate) fn from_archetype(other: &Self) -> Self {
        Self {
            components: other
                .components
                .iter()
                .map(|(id, c)| (*id, c.meta().instantiate()))
                .collect::<Vec<_>>(),
            entities: Vec::new(),
            row_capacity: 0,
        }
    }

    /// creates a new archetype with the given entity + components.
    pub(crate) fn from_row(
        components: &Components,
        entity: Entity,
        component_boxes: Vec<ComponentBox>,
    ) -> Self {
        let mut columns = Vec::with_capacity(component_boxes.len());
        for comp_box in component_boxes {
            let mut column = components.storage_meta_of_id(comp_box.id).instantiate();
            column.reserve(Self::INITIAL_ROWS);
            let data_ptr = Box::into_raw(comp_box.data) as *mut u8;
            unsafe { column.push_from_ptr_unchecked(data_ptr) };
            columns.push((comp_box.id, column));
        }

        let mut entities = Vec::with_capacity(Self::INITIAL_ROWS);
        entities.push(entity);
        Self {
            components: columns,
            entities,
            row_capacity: Self::INITIAL_ROWS,
        }
    }

    pub(crate) fn reserve_rows(&mut self, additional: usize) {
        self.row_capacity += additional;
        self.entities.reserve(additional);
        for (_, column) in self.components.iter_mut() {
            column.reserve(additional);
        }
    }

    fn grow_rows(&mut self) {
        let additional = self.rows_len().max(Self::INITIAL_ROWS);
        self.reserve_rows(additional);
    }

    // --- ecs access ---
    pub(crate) fn iter<Q: Queryable>(
        &mut self,
        component_ids: Q::ComponentIds,
    ) -> ArchetypeIter<Q> {
        let row_count = self.rows_len();
        let columns = Q::fetch_columns(self, component_ids);
        ArchetypeIter::new(columns, row_count)
    }

    // --- meta ---
    /// add column to archetype in place
    pub(crate) fn add_column<T: Component>(&mut self, component_id: ComponentId) {
        debug_assert!(
            self.try_get_column_index(component_id).is_none(),
            "Component {} already exists in archetype",
            type_name::<T>()
        );
        self.components.push((component_id, BlobVec::new::<T>()));
    }

    /// remove column from archetype in place
    pub(crate) fn remove_column(&mut self, component_id: ComponentId) {
        debug_assert!(
            self.try_get_column_index(component_id).is_some(),
            "ComponentId {} does not exist in archetype",
            component_id
        );
        let idx = self.get_column_index(component_id);
        self.components.swap_remove(idx);
    }

    // --- data ---
    /// push row of entity + components to archetype
    pub(crate) fn push_row(&mut self, entity: Entity, component_boxes: Vec<ComponentBox>) {
        if self.rows_len() >= self.rows_capacity() {
            self.grow_rows();
        }
        for comp_box in component_boxes {
            let column = self.get_column_by_id_mut(comp_box.id);
            let data_ptr = Box::into_raw(comp_box.data) as *mut u8;
            unsafe { column.push_from_ptr_unchecked(data_ptr) };
        }
        self.entities.push(entity);
    }

    /// Swap-removes a row from this archetype and moves it into `dst`.
    ///
    /// `skip_comp_id` should be `None` when `dst` has all the same components as `self`,
    /// or `Some(id)` when `dst` is missing exactly one component, to skip copying it.
    ///
    /// Returns the id of the entity that was swapped into `row` to fill the gap, or `None`
    /// if the removed row was the last one.
    pub(crate) fn move_row_into(
        &mut self,
        row: usize,
        dst: &mut Self,
        skip_comp_id: Option<ComponentId>,
    ) -> Option<Entity> {
        if dst.rows_len() >= dst.rows_capacity() {
            dst.grow_rows();
        }
        for (comp_id, column) in self.components.iter_mut() {
            if skip_comp_id == Some(*comp_id) {
                column.swap_remove(row);
                continue;
            }
            let dst_column = dst.get_column_by_id_mut(*comp_id);
            unsafe {
                let ptr = dst_column.push_uninit_unchecked();
                column.swap_remove_into(row, ptr);
            }
        }
        let entity = self.entities.swap_remove(row);
        dst.entities.push(entity);
        self.entities.get(row).copied()
    }

    // --- meta ---
    pub fn rows_len(&self) -> usize {
        self.entities.len()
    }

    pub fn rows_capacity(&self) -> usize {
        self.row_capacity
    }

    // --- data ---
    pub(crate) fn set<T: Component>(&mut self, row: usize, value: T, component_id: ComponentId) {
        *self
            .get_column_by_id_mut(component_id)
            .get_mut(row)
            .unwrap() = value;
    }

    pub fn get_column_by_id(&self, component_id: ComponentId) -> &BlobVec {
        let col_idx = self.get_column_index(component_id);
        &self.components[col_idx].1
    }

    pub fn get_column_by_id_mut(&mut self, component_id: ComponentId) -> &mut BlobVec {
        let col_idx = self.get_column_index(component_id);
        &mut self.components[col_idx].1
    }

    // --- private ---
    #[inline]
    fn try_get_column_index(&self, comp_id: ComponentId) -> Option<usize> {
        self.components.iter().position(|(id, _)| *id == comp_id)
    }

    #[inline]
    fn get_column_index(&self, comp_id: ComponentId) -> usize {
        match self.try_get_column_index(comp_id) {
            Some(idx) => idx,
            None => panic!("ComponentId {} not found in archetype", comp_id), // todo comp type name instead of id
        }
    }
}

pub struct ArchetypeIter<Q: Queryable> {
    columns: Q::ColumnTuple,
    index: usize,
    end: usize,
}

impl<Q: Queryable> ArchetypeIter<Q> {
    pub(crate) fn new(columns: Q::ColumnTuple, end: usize) -> Self {
        Self {
            columns,
            index: 0,
            end,
        }
    }

    pub(crate) fn next(&mut self) -> Option<Q::IterTuple<'_>> {
        if self.index < self.end {
            let item = Q::fetch_row(self.columns, self.index);
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

pub struct Archetypes {
    ids: FxHashMap<ArchetypeKey, ArchetypeId>,
    dense: Vec<Archetype>,
    dense_keys: Vec<ArchetypeKey>,
}

impl Archetypes {
    pub fn new() -> Self {
        let mut this = Self {
            ids: FxHashMap::default(),
            dense: Vec::new(),
            dense_keys: Vec::new(),
        };
        this.register(ArchetypeKey::EMPTY, Archetype::new());
        this
    }

    pub(crate) fn query<Q: Queryable, F: QueryFilter>(
        &mut self,
        with: ArchetypeKey,
        without: ArchetypeKey,
        component_ids: Q::ComponentIds,
    ) -> Query<Q, F> {
        let mut iters: Vec<ArchetypeIter<Q>> = Vec::new();
        for i in 0..self.dense_keys.len() {
            if self.dense_keys[i].matches(&with, &without) {
                let arch = unsafe { self.dense.get_unchecked_mut(i) };
                iters.push(arch.iter::<Q>(component_ids));
            }
        }
        Query::new(iters)
    }

    pub(crate) fn query_state<Q: Queryable, F: QueryFilter>(
        &mut self,
        state: &mut QueryState<Q>,
    ) -> Query<Q, F> {
        state.update_matched(&self.dense_keys);
        let mut iters: Vec<ArchetypeIter<Q>> = Vec::with_capacity(state.matched().len());
        let component_ids = state.component_ids;
        for &id in state.matched() {
            let arch = unsafe { self.dense.get_unchecked_mut(id) };
            iters.push(arch.iter::<Q>(component_ids));
        }
        Query::new(iters)
    }

    pub(crate) fn get_by_id(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.dense.get(id)
    }

    pub(crate) fn get_by_id_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.dense.get_mut(id)
    }

    pub(crate) unsafe fn get_two_by_ids_mut(
        &mut self,
        id1: ArchetypeId,
        id2: ArchetypeId,
    ) -> [&mut Archetype; 2] {
        unsafe { self.dense.get_disjoint_unchecked_mut([id1, id2]) }
    }

    pub(crate) fn id_of(&self, key: ArchetypeKey) -> Option<ArchetypeId> {
        self.ids.get(&key).copied()
    }

    pub(crate) fn key_of(&self, id: ArchetypeId) -> Option<&ArchetypeKey> {
        self.dense_keys.get(id)
    }

    pub(crate) fn id_of_or_create_from(
        &mut self,
        old_id: ArchetypeId,
        new_key: ArchetypeKey,
        configure: impl FnOnce(&mut Archetype),
    ) -> ArchetypeId {
        if let Some(id) = self.id_of(new_key) {
            return id;
        }
        let mut arch = Archetype::from_archetype(self.get_by_id(old_id).unwrap());
        configure(&mut arch);
        self.register(new_key, arch)
    }

    pub(crate) fn register(&mut self, key: ArchetypeKey, arch: Archetype) -> ArchetypeId {
        let id = self.dense.len();
        let None = self.ids.insert(key, id) else {
            unreachable!("Attempted to register archetype twice: {:?}", key);
        };
        self.dense_keys.push(key);
        self.dense.push(arch);
        id
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&ArchetypeKey, &Archetype)> {
        self.dense_keys.iter().zip(self.dense.iter())
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (&ArchetypeKey, &mut Archetype)> {
        self.dense_keys.iter().zip(self.dense.iter_mut())
    }
}

#[cfg(debug_assertions)]
impl Debug for Archetype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Archetype {{\n")?;
        write!(f, "    components: [")?;
        for (i, comp_name) in self
            .components
            .iter()
            .map(|(_, comp)| comp.type_name())
            .enumerate()
        {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", comp_name)?;
        }
        write!(f, "],\n")?;
        write!(f, "    entity_ids: [")?;
        for (i, entity) in self.entities.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", entity)?;
        }
        write!(f, "]\n}}")
    }
}
