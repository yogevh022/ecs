use crate::component;
use crate::component::{ARCHETYPE_KEY_WORD_BITS, ARCHETYPE_KEY_WORDS, Component, ComponentId};
use crate::ecs::{ComponentBox, Entity};
use crate::query::{QueryIter, Queryable};
use blobvec::BlobVec;
use rustc_hash::FxHashMap;
use std::any::type_name;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::marker::PhantomData;

pub type ArchetypeId = usize;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ArchetypeKey(pub(crate) [usize; ARCHETYPE_KEY_WORDS as usize]);
impl ArchetypeKey {
    pub const EMPTY: ArchetypeKey = ArchetypeKey([0; ARCHETYPE_KEY_WORDS as usize]);

    #[inline]
    pub fn with<T: Component>(self) -> Self {
        self.with_id(T::component_id())
    }

    #[inline]
    pub fn without<T: Component>(self) -> Self {
        self.without_id(T::component_id())
    }

    #[inline]
    pub fn with_id(mut self, comp_id: ComponentId) -> Self {
        let component_bit = comp_id - 1;
        let bit = component_bit % ARCHETYPE_KEY_WORD_BITS;
        let word = component_bit / ARCHETYPE_KEY_WORD_BITS;
        self.0[word as usize] |= 1 << bit;
        self
    }

    #[inline]
    pub fn without_id(mut self, comp_id: ComponentId) -> Self {
        let component_bit = comp_id - 1;
        let bit = component_bit % ARCHETYPE_KEY_WORD_BITS;
        let word = component_bit / ARCHETYPE_KEY_WORD_BITS;
        self.0[word as usize] &= !(1 << bit);
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

    pub fn component_count(&self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
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
    pub(crate) fn from_row(entity: Entity, component_boxes: Vec<ComponentBox>) -> Self {
        let comp_reg = component::registry();
        let mut components = Vec::with_capacity(component_boxes.len());
        for comp_box in component_boxes {
            let mut column = comp_reg.storage_meta_of_id(comp_box.id).instantiate();
            column.reserve(4); // fixme arbitrary
            let data_ptr = Box::into_raw(comp_box.data) as *mut u8;
            unsafe { column.push_from_ptr_unchecked(data_ptr) };
            components.push((comp_box.id, column));
        }

        Self {
            components,
            entities: vec![entity],
            row_capacity: 0,
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
        let additional = self.rows_len().max(4);
        self.reserve_rows(additional);
    }

    // --- ecs access ---
    pub(crate) fn iter<Q: Queryable>(&mut self) -> ArchetypeIter<Q> {
        let row_count = self.rows_len();
        let columns = Q::fetch_columns(self);
        ArchetypeIter::new(columns, row_count)
    }

    // --- meta ---
    /// add column to archetype in place
    pub(crate) fn add_column<T: Component>(&mut self) {
        let comp_id = T::component_id();
        debug_assert!(
            self.try_get_column_index(comp_id).is_none(),
            "Component {} already exists in archetype",
            type_name::<T>()
        );
        self.components.push((comp_id, BlobVec::new::<T>()));
    }

    /// remove column from archetype in place
    pub(crate) fn remove_column<T: Component>(&mut self) {
        let comp_id = T::component_id();
        debug_assert!(
            self.try_get_column_index(comp_id).is_some(),
            "Component {} does not exist in archetype",
            type_name::<T>()
        );
        let idx = self.get_column_index(comp_id);
        self.components.swap_remove(idx);
    }

    // --- data ---
    /// push row of entity + components to archetype
    pub(crate) fn push_row(&mut self, entity: Entity, components: Vec<ComponentBox>) {
        if self.rows_len() >= self.rows_capacity() {
            self.grow_rows();
        }
        for comp_box in components {
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
}

impl Archetype {
    // --- meta ---
    pub fn rows_len(&self) -> usize {
        self.entities.len()
    }

    pub fn rows_capacity(&self) -> usize {
        self.row_capacity
    }

    // --- data ---
    pub fn get_column<T: Component>(&self) -> &BlobVec {
        let col_idx = self.get_column_index(T::component_id());
        &self.components[col_idx].1
    }

    pub fn get_column_mut<T: Component>(&mut self) -> &mut BlobVec {
        let col_idx = self.get_column_index(T::component_id());
        &mut self.components[col_idx].1
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

pub struct ArchetypeRegistry {
    registry: FxHashMap<ArchetypeKey, ArchetypeId>,
    dense: Vec<Archetype>,
    dense_keys: Vec<ArchetypeKey>,
}

impl ArchetypeRegistry {
    pub fn new() -> Self {
        let mut this = Self {
            registry: FxHashMap::default(),
            dense: Vec::new(),
            dense_keys: Vec::new(),
        };
        this.register(ArchetypeKey::EMPTY, Archetype::new());
        this
    }

    pub(crate) fn query<WITH: Queryable<Key = ArchetypeKey>>(&mut self) -> QueryIter<WITH> {
        let mut iters: Vec<ArchetypeIter<WITH>> = Vec::new();
        let with_key = WITH::key();
        for i in 0..self.dense_keys.len() {
            let arch_key = &self.dense_keys[i];
            if arch_key.contains(&with_key) {
                let arch = unsafe { self.dense.get_unchecked_mut(i) };
                iters.push(arch.iter());
            }
        }
        QueryIter::new(iters)
    }

    pub(crate) fn query_specific<
        WITH: Queryable<Key = ArchetypeKey>,
        WITHOUT: Queryable<Key = ArchetypeKey>,
    >(
        &mut self,
    ) -> QueryIter<WITH> {
        let mut iters: Vec<ArchetypeIter<WITH>> = Vec::new();
        let with_key = WITH::key();
        let without_key = WITHOUT::key();
        for i in 0..self.dense_keys.len() {
            let arch_key = &self.dense_keys[i];
            if arch_key.contains(&with_key) && arch_key.disjoint(&without_key) {
                let arch = unsafe { self.dense.get_unchecked_mut(i) };
                iters.push(arch.iter());
            }
        }
        QueryIter::new(iters)
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
        self.registry.get(&key).copied()
    }

    pub(crate) fn key_of(&self, id: ArchetypeId) -> Option<&ArchetypeKey> {
        self.dense_keys.get(id)
    }

    pub(crate) fn register_or_push_row(
        &mut self,
        entity: Entity,
        key: ArchetypeKey,
        components: Vec<ComponentBox>,
    ) -> (ArchetypeId, usize) {
        match self.registry.entry(key) {
            Entry::Occupied(e) => {
                let arch_id = *e.get();
                let arch = unsafe {
                    // SAFETY: key in registry == id in dense
                    self.dense.get_unchecked_mut(arch_id)
                };
                let row = arch.rows_len();
                arch.push_row(entity, components);
                (arch_id, row)
            }
            Entry::Vacant(v) => {
                let arch = Archetype::from_row(entity, components);
                let id = self.dense.len();
                self.dense_keys.push(key);
                self.dense.push(arch);
                v.insert(id);
                (id, 0)
            }
        }
    }

    pub(crate) fn id_of_or_register_with<F: FnOnce() -> Archetype>(
        &mut self,
        key: ArchetypeKey,
        default: F,
    ) -> ArchetypeId {
        *self.registry.entry(key).or_insert_with(|| {
            let id = self.dense.len();
            self.dense_keys.push(key);
            self.dense.push(default());
            id
        })
    }

    pub(crate) fn register(&mut self, key: ArchetypeKey, arch: Archetype) -> ArchetypeId {
        let id = self.dense.len();
        let None = self.registry.insert(key, id) else {
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
