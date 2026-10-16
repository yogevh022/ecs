use crate::archetype::ArchetypeId;
use std::fmt::{Debug, Display};

pub(crate) type EntityId = u32;
pub(crate) type EntityGeneration = u32;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Entity {
    pub(crate) id: EntityId,
    pub(crate) generation: EntityGeneration,
}

impl Entity {
    pub(crate) fn new(id: EntityId, generation: EntityGeneration) -> Self {
        Self { id, generation }
    }
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

#[derive(Debug, Copy, Clone)]
pub(crate) struct SparseEntity {
    pub archetype_id: ArchetypeId,
    pub generation: EntityGeneration,
    pub row: u32,
}

pub struct Entities {
    next_id: EntityId,
    free_list: Vec<EntityId>,
    live: Vec<Entity>,
    sparse: Vec<Option<SparseEntity>>,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            free_list: Vec::new(),
            live: Vec::new(),
            sparse: vec![None], // reserved 0 for null entity
        }
    }

    pub(crate) fn alloc(&mut self) -> Entity {
        if let Some(id) = self.free_list.pop() {
            let generation = match self.sparse.get(id as usize).and_then(Option::as_ref) {
                Some(location) => location.generation,
                None => panic!("Entity {id} does not exist in ECS"),
            };
            Entity::new(id, generation)
        } else {
            let id = self.next_id;
            self.next_id += 1;
            Entity::new(id, 0)
        }
    }

    pub(crate) fn free(&mut self, entity: Entity) {
        let Some(location) = self
            .sparse
            .get_mut(entity.id as usize)
            .and_then(Option::as_mut)
        else {
            panic!("Entity {} does not exist in ECS", entity.id);
        };
        if location.generation != entity.generation {
            panic!("Entity {} does not exist in ECS", entity.id);
        }
        location.generation = location.generation.wrapping_add(1);
        self.free_list.push(entity.id);
    }

    pub(crate) fn insert(&mut self, entity: Entity, location: SparseEntity) {
        let id = entity.id as usize;
        let slot = Some(location);
        if id >= self.sparse.len() {
            self.sparse.push(slot);
        } else {
            self.sparse[id] = slot;
        }
        self.live.push(entity);
    }

    pub(crate) fn location(&self, entity: Entity) -> SparseEntity {
        debug_assert!(
            entity.id < self.sparse.len() as u32,
            "Entity {} does not exist in ECS",
            entity.id
        );
        let sparse_slot = unsafe {
            // SAFETY: any entity.id is a valid sparse entity index, even if entity was despawned
            self.sparse.get(entity.id as usize).unwrap_unchecked()
        };
        match sparse_slot {
            Some(location) if location.generation == entity.generation => *location,
            _ => panic!("Entity {} does not exist in ECS", entity.id),
        }
    }

    pub(crate) unsafe fn location_mut(&mut self, entity_id: EntityId) -> &mut SparseEntity {
        unsafe {
            self.sparse
                .get_unchecked_mut(entity_id as usize)
                .as_mut()
                .unwrap_unchecked()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert(entities: &mut Entities, entity: Entity, row: u32) {
        entities.insert(
            entity,
            SparseEntity {
                archetype_id: 0,
                generation: entity.generation,
                row,
            },
        );
    }

    #[test]
    fn recycled_id_bumps_only_its_generation() {
        let mut entities = Entities::new();
        let first = entities.alloc();
        insert(&mut entities, first, 0);
        let second = entities.alloc();
        insert(&mut entities, second, 1);

        assert_eq!(first.generation, 0);
        assert_eq!(second.generation, 0);

        entities.free(first);
        let recycled = entities.alloc();

        assert_eq!(recycled.id, first.id);
        assert_eq!(recycled.generation, 1);
        assert_eq!(second.generation, 0);
    }
}
