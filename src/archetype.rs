use crate::component::{ARCHETYPE_KEY_WORD_BITS, ARCHETYPE_KEY_WORDS, Component, ComponentId};
use crate::world::{Entity, EntityId};
use blobvec::BlobVec;
use rustc_hash::FxHashMap;
use std::any::type_name;
use std::fmt::Debug;

pub type ArchetypeId = usize;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ArchetypeKey(pub(crate) [usize; ARCHETYPE_KEY_WORDS as usize]);
impl ArchetypeKey {
    pub const EMPTY: ArchetypeKey = ArchetypeKey([0; ARCHETYPE_KEY_WORDS as usize]);

    #[inline]
    pub fn with<T: Component>(mut self) -> Self {
        let component_bit = T::component_id() - 1;
        let bit = component_bit % ARCHETYPE_KEY_WORD_BITS;
        let word = component_bit / ARCHETYPE_KEY_WORD_BITS;
        self.0[word as usize] |= 1 << bit;
        self
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

#[derive(Debug)]
pub struct Archetype {
    components: Vec<BlobVec>,
    component_ids: Vec<ComponentId>,
    entity_ids: Vec<EntityId>,
    row_capacity: usize,
}

impl Archetype {
    // --- construction ---
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            component_ids: Vec::new(),
            entity_ids: Vec::new(),
            row_capacity: 0,
        }
    }

    pub fn clone_empty(&self) -> Self {
        Self {
            components: self
                .components
                .iter()
                .map(|c| c.clone_empty())
                .collect::<Vec<_>>(),
            component_ids: self.component_ids.clone(),
            entity_ids: Vec::new(),
            row_capacity: 0,
        }
    }

    pub fn reserve_rows(&mut self, additional: usize) {
        self.row_capacity += additional;
        self.entity_ids.reserve(additional);
        for column in self.components.iter_mut() {
            column.reserve(additional);
        }
    }

    fn grow_rows(&mut self) {
        let additional = self.rows_len().max(4);
        self.reserve_rows(additional);
    }

    // --- accessors ---
    pub fn get_column<T: Component>(&self) -> &BlobVec {
        match self.get_column_index::<T>() {
            Some(col_idx) => &self.components[col_idx],
            None => panic!("Component {} not found in archetype", type_name::<T>()),
        }
    }

    pub fn get_column_mut<T: Component>(&mut self) -> &mut BlobVec {
        match self.get_column_index::<T>() {
            Some(col_idx) => &mut self.components[col_idx],
            None => panic!("Component {} not found in archetype", type_name::<T>()),
        }
    }

    pub fn get_column_by_id(&self, component_id: ComponentId) -> &BlobVec {
        match self.get_column_index_by_id(component_id) {
            Some(col_idx) => &self.components[col_idx],
            None => panic!("ComponentId {} not found in archetype", component_id),
        }
    }

    pub fn get_column_by_id_mut(&mut self, component_id: ComponentId) -> &mut BlobVec {
        match self.get_column_index_by_id(component_id) {
            Some(col_idx) => &mut self.components[col_idx],
            None => panic!("ComponentId {} not found in archetype", component_id),
        }
    }

    pub fn rows_len(&self) -> usize {
        self.entity_ids.len()
    }

    pub fn rows_capacity(&self) -> usize {
        self.row_capacity
    }

    // --- insertion ---
    pub(crate) fn add_column<T: Component>(&mut self) {
        debug_assert!(
            self.get_column_index::<T>().is_none(),
            "Component {} already exists in archetype",
            type_name::<T>()
        );
        self.components.push(BlobVec::new::<T>());
        self.component_ids.push(T::component_id());
    }

    /// swap-remove a row from this archetype and insert it into another.
    /// the other archetype must contain at least all components from this archetype.
    /// if an entity was swapped, returns its id, otherwise returns None
    pub(crate) fn move_row_into(&mut self, row: usize, dst: &mut Self) -> Option<EntityId> {
        if dst.rows_len() >= dst.rows_capacity() {
            dst.grow_rows();
        }
        for (comp_id, column) in self.iter_columns_mut() {
            let dst_column = dst.get_column_by_id_mut(comp_id);
            unsafe {
                let ptr = dst_column.push_uninit_unchecked();
                column.swap_remove_into(row, ptr);
            }
        }
        let entity = self.entity_ids.swap_remove(row);
        dst.entity_ids.push(entity);
        self.entity_ids.get(row).copied()
    }

    pub(crate) fn iter_columns(&self) -> impl Iterator<Item = (ComponentId, &BlobVec)> {
        self.component_ids
            .iter()
            .copied()
            .zip(self.components.iter())
    }

    pub(crate) fn iter_columns_mut(&mut self) -> impl Iterator<Item = (ComponentId, &mut BlobVec)> {
        self.component_ids
            .iter()
            .copied()
            .zip(self.components.iter_mut())
    }

    pub(crate) fn push_entity_id(&mut self, entity: EntityId) {
        self.entity_ids.push(entity);
    }

    // --- private ---
    #[inline]
    fn get_column_index<T: Component>(&self) -> Option<usize> {
        let comp_id = T::component_id();
        self.get_column_index_by_id(comp_id)
    }

    #[inline]
    fn get_column_index_by_id(&self, comp_id: ComponentId) -> Option<usize> {
        self.component_ids.iter().position(|id| *id == comp_id)
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

    pub fn debug(&self) {
        dbg!(&self.registry);
        dbg!(&self.dense);
        dbg!(&self.dense_keys);
    }

    pub(crate) fn get_by_id(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.dense.get(id)
    }

    pub(crate) fn get_by_id_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.dense.get_mut(id)
    }

    pub(crate) unsafe fn get_two_by_ids_mut(&mut self, id1: ArchetypeId, id2: ArchetypeId) -> [&mut Archetype; 2] {
        unsafe { self.dense.get_disjoint_unchecked_mut([id1, id2]) }
    }

    pub(crate) fn id_of(&self, key: ArchetypeKey) -> Option<ArchetypeId> {
        self.registry.get(&key).copied()
    }

    pub(crate) fn key_of(&self, id: ArchetypeId) -> Option<&ArchetypeKey> {
        self.dense_keys.get(id)
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

// impl Archetype {
//     // --- construction ---
//     pub fn new() -> Self {
//         Self {
//             components: Vec::new(),
//             component_ids: Vec::new(),
//             entity_ids: Vec::new(),
//         }
//     }
//
//     pub fn clone_empty(&self) -> Self {
//         Self {
//             components: self
//                 .components
//                 .iter()
//                 .map(|c| c.clone_empty())
//                 .collect::<Vec<_>>(),
//             component_ids: self.component_ids.clone(),
//             entity_ids: Vec::new(),
//         }
//     }
//
//     // --- accessors ---
//     pub fn get_column<T: Component>(&self) -> &BlobVec {
//         match self.get_column_index::<T>() {
//             Some(col_idx) => &self.components[col_idx],
//             None => panic!("Component {} not found in archetype", type_name::<T>()),
//         }
//     }
//
//     pub fn get_column_mut<T: Component>(&mut self) -> &mut BlobVec {
//         match self.get_column_index::<T>() {
//             Some(col_idx) => &mut self.components[col_idx],
//             None => panic!("Component {} not found in archetype", type_name::<T>()),
//         }
//     }
//
//     pub fn get_column_by_id(&self, component_id: ComponentId) -> &BlobVec {
//         match self.get_column_index_by_id(component_id) {
//             Some(col_idx) => &self.components[col_idx],
//             None => panic!("ComponentId {} not found in archetype", component_id),
//         }
//     }
//
//     pub fn get_column_by_id_mut(&mut self, component_id: ComponentId) -> &mut BlobVec {
//         match self.get_column_index_by_id(component_id) {
//             Some(col_idx) => &mut self.components[col_idx],
//             None => panic!("ComponentId {} not found in archetype", component_id),
//         }
//     }
//
//     pub fn rows(&self) -> usize {
//         self.entity_ids.len()
//     }
//
//     // --- insertion ---
//     pub(crate) fn add_column<T: Component>(&mut self) {
//         debug_assert!(
//             self.get_column_index::<T>().is_none(),
//             "Component {} already exists in archetype",
//             type_name::<T>()
//         );
//         let column = BlobVec::new::<T>();
//         self.components.push(column);
//         self.component_ids.push(T::component_id());
//     }
//
//     /// swap removes a row, copies it into the other archetype
//     pub(crate) fn swap_row_into(&mut self, other: &mut Self, row: usize) -> Option<Entity> {
//         for (column, comp_id) in self.components.iter_mut().zip(self.component_ids.iter()) {
//             let other_column = other.get_column_by_id_mut(*comp_id);
//             column.swap_pop_into(other_column, row);
//         }
//         let entity = self.entity_ids.swap_remove(row);
//         other.entity_ids.push(entity);
//         self.entity_ids.get(row).copied()
//     }
//
//     pub(crate) fn push_entity(&mut self, entity: Entity) {
//         self.entity_ids.push(entity);
//     }
//
//     // --- private ---
//     #[inline]
//     fn get_column_index<T: Component>(&self) -> Option<usize> {
//         let comp_id = T::component_id();
//         self.get_column_index_by_id(comp_id)
//     }
//
//     #[inline]
//     fn get_column_index_by_id(&self, comp_id: ComponentId) -> Option<usize> {
//         self.component_ids.iter().position(|id| *id == comp_id)
//     }
// }
