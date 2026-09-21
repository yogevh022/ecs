use crate::archetype::{Archetype, ArchetypeId, ArchetypeKey, Archetypes};
use crate::component::{Component, ComponentId};
use crate::query::{QueryFilter, QueryIter, Queryable};
use std::any::{Any, type_name};
use std::fmt::{Debug, Display};

pub type EntityId = u32;

#[derive(Clone, Copy)]
pub struct Entity {
    pub id: EntityId,
    generation: u32,
}

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({})", self.id)
    }
}

impl Debug for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Entity {
    pub fn new(id: EntityId, generation: u32) -> Self {
        Self { id, generation }
    }
}

struct EntityAllocator {
    counter: u32,
    generation: u32,
    free_list: Vec<EntityId>,
}

impl EntityAllocator {
    fn new() -> Self {
        Self {
            counter: 1,
            generation: 0,
            free_list: Vec::new(),
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        let mut this = Self::new();
        this.free_list.reserve(capacity);
        this
    }

    fn alloc(&mut self) -> Entity {
        if let Some(id) = self.free_list.pop() {
            self.generation += 1;
            Entity::new(id, self.generation)
        } else {
            let id = self.counter;
            self.counter += 1;
            Entity::new(id, self.generation)
        }
    }

    fn free(&mut self, entity: Entity) {
        self.free_list.push(entity.id);
    }
}

pub(crate) struct ComponentBox {
    pub id: ComponentId,
    pub data: Box<dyn Any>,
}

pub struct EntityBuilder<'e> {
    ecs: &'e mut Ecs,
    components: Vec<ComponentBox>,
}

impl<'e> EntityBuilder<'e> {
    fn new(ecs: &'e mut Ecs) -> Self {
        Self {
            ecs,
            components: Vec::with_capacity(16), // arbitrary
        }
    }

    pub fn with<T: Component>(mut self, component: T) -> Self {
        self.components.push(ComponentBox {
            id: T::component_id(),
            data: Box::new(component),
        });
        self
    }

    pub fn spawn(self) -> Entity {
        self.ecs.spawn_with_components(self.components)
    }
}

struct EntityPrefab {
    components: Vec<ComponentBox>,
}

impl EntityPrefab {
    pub fn from(components: Vec<ComponentBox>) -> Self {
        Self { components }
    }
}

#[derive(Debug, Copy, Clone)]
struct SparseEntity {
    archetype_id: ArchetypeId,
    generation: u32,
    row: u32,
}

pub struct Ecs {
    entity_allocator: EntityAllocator,
    entities: Vec<Entity>,
    entities_sparse: Vec<Option<SparseEntity>>,
    archetypes: Archetypes,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            entity_allocator: EntityAllocator::new(),
            entities: Vec::new(),
            entities_sparse: vec![None], // reserved 0 for null entity
            archetypes: Archetypes::new(),
        }
    }

    pub fn query_filtered<Q: Queryable, F: QueryFilter>(&mut self) -> QueryIter<Q> {
        self.archetypes.query_filtered::<Q, F>()
    }

    pub fn query<Q: Queryable>(&mut self) -> QueryIter<Q> {
        self.query_filtered::<Q, ()>()
    }

    pub fn new_entity(&'_ mut self) -> EntityBuilder<'_> {
        EntityBuilder::new(self)
    }

    pub fn spawn_prefab(&mut self, prefab: &EntityPrefab) -> Entity {
        todo!()
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let sparse_entity = self.get_sparse_entity(entity);
        let old_id = sparse_entity.archetype_id;
        let old_row = sparse_entity.row;
        // SAFETY: every entity belongs to an archetype
        let old_key = unsafe { *self.archetypes.key_of(old_id as _).unwrap_unchecked() };
        let new_key = old_key.with::<T>();

        if old_key == new_key {
            let arch = self.archetypes.get_by_id_mut(old_id).unwrap();
            arch.set(old_row as usize, component);
            return;
        }

        let new_id =
            self.archetypes
                .id_of_or_create_from(old_id, new_key, Archetype::add_column::<T>);

        let entity_new_row = unsafe {
            // SAFETY: both old_id and new_id archetypes exist, confirmed above
            self.migrate_columns_with::<T>(old_id, new_id, old_row, component)
        };

        let sparse_entity = unsafe { self.get_sparse_slot_unchecked_mut(entity.id) };
        sparse_entity.archetype_id = new_id;
        sparse_entity.row = entity_new_row;
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        let sparse_entity = self.get_sparse_entity(entity);
        let old_id = sparse_entity.archetype_id;
        let old_row = sparse_entity.row;
        // SAFETY: every entity belongs to an archetype
        let old_key = unsafe { *self.archetypes.key_of(old_id as _).unwrap_unchecked() };
        let new_key = old_key.without::<T>();

        if old_key == new_key {
            return; // component does not exist
        }

        let new_id =
            self.archetypes
                .id_of_or_create_from(old_id, new_key, Archetype::remove_column::<T>);

        let entity_new_row = unsafe {
            // SAFETY: both old_id and new_id archetypes exist, confirmed above
            self.migrate_columns_without::<T>(old_id, new_id, old_row)
        };

        let sparse_entity = unsafe { self.get_sparse_slot_unchecked_mut(entity.id) };
        sparse_entity.archetype_id = new_id;
        sparse_entity.row = entity_new_row;
    }

    // --- private ---
    fn spawn_with_components(&mut self, components: Vec<ComponentBox>) -> Entity {
        let mut key = ArchetypeKey::EMPTY;
        for comp_box in &components {
            key = key.with_id(comp_box.id);
        }
        debug_assert_eq!(
            key.component_count(),
            components.len(),
            "Duplicate components found"
        );

        let entity = self.entity_allocator.alloc();
        let (arch_id, row) = self
            .archetypes
            .register_or_push_row(entity, key, components);
        let sparse_entity = Some(SparseEntity {
            archetype_id: arch_id,
            generation: entity.generation,
            row: row as u32,
        });
        let entity_id = entity.id as usize;
        if entity_id >= self.entities_sparse.len() {
            self.entities_sparse.push(sparse_entity)
        } else {
            self.entities_sparse[entity_id] = sparse_entity;
        }
        self.entities.push(entity);
        entity
    }

    unsafe fn migrate_columns_with<T: Component>(
        &mut self,
        src_id: ArchetypeId,
        dst_id: ArchetypeId,
        src_row: u32,
        component: T,
    ) -> u32 {
        let [old_arch, new_arch] = unsafe { self.archetypes.get_two_by_ids_mut(src_id, dst_id) };

        let entity_new_row = new_arch.rows_len() as u32;
        new_arch.get_column_mut::<T>().push(component);
        if let Some(swapped_entity) = old_arch.move_row_into(src_row as usize, new_arch, None) {
            // SAFETY: if swapped_entity is Some, it must be a valid sparse entity index
            unsafe { self.get_sparse_slot_unchecked_mut(swapped_entity.id).row = src_row };
        }
        entity_new_row
    }

    unsafe fn migrate_columns_without<T: Component>(
        &mut self,
        src_id: ArchetypeId,
        dst_id: ArchetypeId,
        src_row: u32,
    ) -> u32 {
        let [old_arch, new_arch] = unsafe { self.archetypes.get_two_by_ids_mut(src_id, dst_id) };

        let entity_new_row = new_arch.rows_len() as u32;
        if let Some(swapped_entity) =
            old_arch.move_row_into(src_row as usize, new_arch, Some(T::component_id()))
        {
            // SAFETY: if swapped_entity is Some, it must be a valid sparse entity index
            unsafe { self.get_sparse_slot_unchecked_mut(swapped_entity.id).row = src_row };
        }
        entity_new_row
    }

    fn get_sparse_entity(&self, entity: Entity) -> SparseEntity {
        debug_assert!(
            entity.id < self.entities_sparse.len() as u32,
            "Entity {} does not exist in ECS",
            entity.id
        );
        let sparse_slot = unsafe {
            // SAFETY: any entity.id is a valid sparse entity index, even if entity was despawned
            self.entities_sparse
                .get(entity.id as usize)
                .unwrap_unchecked()
        };
        match sparse_slot {
            Some(sparse_entity) if sparse_entity.generation == entity.generation => *sparse_entity,
            _ => panic!("Entity {} does not exist in ECS", entity.id),
        }
    }

    unsafe fn get_sparse_slot_unchecked_mut(&mut self, entity_id: EntityId) -> &mut SparseEntity {
        unsafe {
            self.entities_sparse
                .get_unchecked_mut(entity_id as usize)
                .as_mut()
                .unwrap_unchecked()
        }
    }
}
