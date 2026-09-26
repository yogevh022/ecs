use crate::archetype::{Archetype, ArchetypeId, ArchetypeKey, Archetypes};
use crate::component::{Component, ComponentId, Components};
use crate::entity::{Entities, Entity, SparseEntity};
use crate::event::{Event, Events};
use crate::query::{Query, QueryFilter, QueryState, Queryable};
use std::any::Any;

pub(crate) struct ComponentBox {
    pub id: ComponentId,
    pub data: Box<dyn Any>,
}

pub struct EntityBuilder<'e> {
    ecs: &'e mut World,
    components: Vec<ComponentBox>,
}

impl<'e> EntityBuilder<'e> {
    fn new(ecs: &'e mut World) -> Self {
        Self {
            ecs,
            components: Vec::with_capacity(16), // arbitrary
        }
    }

    pub fn with<T: Component>(mut self, component: T) -> Self {
        self.components.push(ComponentBox {
            id: self.ecs.components.id_of::<T>(),
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

pub struct World {
    entities: Entities,
    pub(crate) components: Components,
    pub(crate) events: Events,
    archetypes: Archetypes,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: Entities::new(),
            components: Components::new(),
            events: Events::new(),
            archetypes: Archetypes::new(),
        }
    }

    pub fn register_component<T: Component>(&mut self) {
        self.components.register::<T>();
    }

    pub fn register_event<E: Event>(&mut self) {
        self.events.register::<E>();
    }

    pub fn new_entity(&'_ mut self) -> EntityBuilder<'_> {
        EntityBuilder::new(self)
    }

    pub fn spawn_prefab(&mut self, prefab: &EntityPrefab) -> Entity {
        todo!()
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let sparse_entity = self.entities.location(entity);
        let old_id = sparse_entity.archetype_id;
        let old_row = sparse_entity.row;
        // SAFETY: every entity belongs to an archetype
        let old_key = unsafe { *self.archetypes.key_of(old_id as _).unwrap_unchecked() };
        let component_id = self.components.id_of::<T>();
        let new_key = old_key.with_id(component_id);

        if old_key == new_key {
            let arch = self.archetypes.get_by_id_mut(old_id).unwrap();
            arch.set(old_row as usize, component, component_id);
            return;
        }

        let new_id = self.archetypes.id_of_or_create_from(old_id, new_key, |arch| {
            arch.add_column::<T>(component_id);
        });

        let entity_new_row = unsafe {
            // SAFETY: both old_id and new_id archetypes exist, confirmed above
            self.migrate_columns_with::<T>(old_id, new_id, old_row, component)
        };

        let sparse_entity = unsafe { self.entities.location_mut(entity.id) };
        sparse_entity.archetype_id = new_id;
        sparse_entity.row = entity_new_row;
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        let sparse_entity = self.entities.location(entity);
        let old_id = sparse_entity.archetype_id;
        let old_row = sparse_entity.row;
        // SAFETY: every entity belongs to an archetype
        let old_key = unsafe { *self.archetypes.key_of(old_id as _).unwrap_unchecked() };
        let component_id = self.components.id_of::<T>();
        let new_key = old_key.without_id(component_id);

        if old_key == new_key {
            return; // component does not exist
        }

        let new_id = self.archetypes.id_of_or_create_from(old_id, new_key, |arch| {
            arch.remove_column(component_id);
        });

        let entity_new_row = unsafe {
            // SAFETY: both old_id and new_id archetypes exist, confirmed above
            self.migrate_columns_without::<T>(old_id, new_id, old_row)
        };

        let sparse_entity = unsafe { self.entities.location_mut(entity.id) };
        sparse_entity.archetype_id = new_id;
        sparse_entity.row = entity_new_row;
    }

    pub fn query<Q: Queryable>(&mut self) -> Query<Q> {
        self.query_filtered::<Q, ()>()
    }

    pub fn query_filtered<Q: Queryable, F: QueryFilter>(&mut self) -> Query<Q, F> {
        let components = &self.components;
        let component_ids = Q::component_ids(components);
        let with = Q::key(component_ids) | F::include(components);
        let without = F::exclude(components);
        self.archetypes
            .query::<Q, F>(with, without, component_ids)
    }

    pub(crate) fn query_state<Q: Queryable, F: QueryFilter>(
        &mut self,
        state: &mut QueryState<Q>,
    ) -> Query<Q, F> {
        self.archetypes.query_state(state)
    }

    // --- private ---
    fn spawn_with_components(&mut self, component_boxes: Vec<ComponentBox>) -> Entity {
        let mut key = ArchetypeKey::EMPTY;
        for comp_box in &component_boxes {
            key = key.with_id(comp_box.id);
        }
        debug_assert_eq!(
            key.component_count(),
            component_boxes.len(),
            "Duplicate components found"
        );

        let entity = self.entities.alloc();
        let (arch_id, row) = if let Some(arch_id) = self.archetypes.id_of(key) {
            let arch = self.archetypes.get_by_id_mut(arch_id).unwrap();
            let row = arch.rows_len();
            arch.push_row(entity, component_boxes);
            (arch_id, row)
        } else {
            let arch = Archetype::from_row(&self.components, entity, component_boxes);
            let arch_id = self.archetypes.register(key, arch);
            (arch_id, 0)
        };
        self.entities.insert(
            entity,
            SparseEntity {
                archetype_id: arch_id,
                generation: entity.generation,
                row: row as u32,
            },
        );
        entity
    }

    unsafe fn migrate_columns_with<T: Component>(
        &mut self,
        src_id: ArchetypeId,
        dst_id: ArchetypeId,
        src_row: u32,
        component: T,
    ) -> u32 {
        let component_id = self.components.id_of::<T>();
        let [old_arch, new_arch] = unsafe { self.archetypes.get_two_by_ids_mut(src_id, dst_id) };

        let entity_new_row = new_arch.rows_len() as u32;
        new_arch.get_column_by_id_mut(component_id).push(component);
        if let Some(swapped_entity) = old_arch.move_row_into(src_row as usize, new_arch, None) {
            // SAFETY: if swapped_entity is Some, it must be a valid sparse entity index
            unsafe { self.entities.location_mut(swapped_entity.id).row = src_row };
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
        if let Some(swapped_entity) = old_arch.move_row_into(
            src_row as usize,
            new_arch,
            Some(self.components.id_of::<T>()),
        ) {
            // SAFETY: if swapped_entity is Some, it must be a valid sparse entity index
            unsafe { self.entities.location_mut(swapped_entity.id).row = src_row };
        }
        entity_new_row
    }
}
