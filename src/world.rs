use crate::archetype::{Archetype, ArchetypeId, ArchetypeKey, ArchetypeRegistry};
use crate::component;
use crate::component::{Component, ComponentId};
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
            components: Vec::new(),
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
        let mut key = ArchetypeKey::EMPTY;
        for comp_box in &self.components {
            key = key.with_id(comp_box.id);
        }
        debug_assert_eq!(
            key.component_count(),
            self.components.len(),
            "Duplicate components found"
        );

        let entity = self.ecs.entity_allocator.alloc();
        let (arch_id, row) = self
            .ecs
            .archetypes
            .register_or_push_row(entity, key, self.components);
        let sparse_entity = Some(SparseEntity {
            archetype_id: arch_id,
            generation: entity.generation,
            row: row as u32,
        });
        let entity_id = entity.id as usize;
        if entity_id >= self.ecs.entities_sparse.len() {
            self.ecs.entities_sparse.push(sparse_entity)
        } else {
            self.ecs.entities_sparse[entity_id] = sparse_entity;
        }
        self.ecs.entities.push(entity);
        entity
    }
}

// impl<'e> EntityBuilder<'e> {
//     fn new(ecs: &'e mut Ecs) -> Self {
//         Self {
//             ecs,
//             components: Vec::new(), // fixme needs sufficient preallocation
//         }
//     }
//
//     fn with(mut self, component: Box<dyn Component>) -> Self {
//         self.components.push(component);
//         self
//     }
//
//     fn build(self) -> Entity {
//         // fixme make faster solution
//         let entity = self.ecs.spawn();
//         let reg = component_registry_read();
//         let component_ids = self
//             .components
//             .iter()
//             .filter_map(|comp| reg.component_id(comp))
//             .collect::<Vec<_>>();
//         let archetype_keys = reg.archetype_key(&component_ids);
//         self.ecs.archetypes.entry(archetype_keys).or_insert_with(|| Archetype::new());
//         for component in self.components {
//             self.ecs.add_component(entity, component);
//         }
//         entity
//     }
// }

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
    archetypes: ArchetypeRegistry,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            entity_allocator: EntityAllocator::new(),
            entities: Vec::new(),
            entities_sparse: vec![None], // reserved 0 for null entity
            archetypes: ArchetypeRegistry::new(),
        }
    }

    pub fn debug(&self) {
        dbg!(&self.entities);
        // dbg!(&self.entities_sparse);
        self.archetypes.debug();
        // dbg!(&self.archetypes);
    }

    pub fn new_entity(&'_ mut self) -> EntityBuilder<'_> {
        EntityBuilder::new(self)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let sparse_entity = self.get_sparse_entity(entity);
        let old_id = sparse_entity.archetype_id;
        let old_row = sparse_entity.row;
        // SAFETY: every entity belongs to an archetype
        let old_key = unsafe { *self.archetypes.key_of(old_id as _).unwrap_unchecked() };
        let new_key = old_key.with::<T>();

        if old_key == new_key {
            panic!("{} already has component {}", entity, type_name::<T>());
        }

        let old_arch_shallow = unsafe {
            // SAFETY: every entity belongs to an archetype
            (self.archetypes.get_by_id(old_id).unwrap_unchecked() as *const Archetype).read()
        };

        let new_id = self.archetypes.id_of_or_register_with(new_key, || {
            let mut new_arch = Archetype::from_archetype(&old_arch_shallow);
            new_arch.add_column::<T>();
            new_arch
        });
        // forget the shallow copy of old_arch to avoid double free
        std::mem::forget(old_arch_shallow);

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
            panic!("{} does not have component {}", entity, type_name::<T>());
        }

        let old_arch_shallow = unsafe {
            // SAFETY: every entity belongs to an archetype
            (self.archetypes.get_by_id(old_id).unwrap_unchecked() as *const Archetype).read()
        };

        let new_id = self.archetypes.id_of_or_register_with(new_key, || {
            let mut new_arch = Archetype::from_archetype(&old_arch_shallow);
            new_arch.remove_column::<T>();
            new_arch
        });
        // forget the shallow copy of old_arch to avoid double free
        std::mem::forget(old_arch_shallow);

        let entity_new_row = unsafe {
            // SAFETY: both old_id and new_id archetypes exist, confirmed above
            self.migrate_columns_without::<T>(old_id, new_id, old_row)
        };

        let sparse_entity = unsafe { self.get_sparse_slot_unchecked_mut(entity.id) };
        sparse_entity.archetype_id = new_id;
        sparse_entity.row = entity_new_row;
    }

    // --- private ---
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
