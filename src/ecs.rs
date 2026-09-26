use crate::archetype::{Archetype, ArchetypeId, ArchetypeKey, Archetypes};
use crate::component::{Component, ComponentId};
use crate::entity::{Entities, Entity, SparseEntity};
use crate::query::{QueryFilter, Query, QueryState, Queryable};
use std::any::Any;
use crate::event::Events;

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

pub struct Ecs {
    entities: Entities,
    pub(crate) events: Events,
    archetypes: Archetypes,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            entities: Entities::new(),
            events: Events::new(),
            archetypes: Archetypes::new(),
        }
    }

    pub fn query_filtered<Q: Queryable, F: QueryFilter>(&mut self) -> Query<Q, F> {
        self.archetypes.query_filtered::<Q, F>()
    }

    pub fn query<Q: Queryable>(&mut self) -> Query<Q> {
        self.query_filtered::<Q, ()>()
    }

    pub(crate) fn query_state<Q: Queryable, F: QueryFilter>(&mut self, state: &QueryState) -> Query<Q, F> {
        self.archetypes.query_state(state)
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

        let sparse_entity = unsafe { self.entities.location_mut(entity.id) };
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

        let entity = self.entities.alloc();
        let (arch_id, row) = self
            .archetypes
            .register_or_push_row(entity, key, components);
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
        let [old_arch, new_arch] = unsafe { self.archetypes.get_two_by_ids_mut(src_id, dst_id) };

        let entity_new_row = new_arch.rows_len() as u32;
        new_arch.get_column_mut::<T>().push(component);
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
        if let Some(swapped_entity) =
            old_arch.move_row_into(src_row as usize, new_arch, Some(T::component_id()))
        {
            // SAFETY: if swapped_entity is Some, it must be a valid sparse entity index
            unsafe { self.entities.location_mut(swapped_entity.id).row = src_row };
        }
        entity_new_row
    }
}
