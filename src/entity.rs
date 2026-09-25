use crate::archetype::ArchetypeId;
use std::fmt::{Debug, Display};

pub type EntityId = u32;

#[derive(Clone, Copy)]
pub struct Entity {
    pub id: EntityId,
    pub(crate) generation: u32,
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

#[derive(Debug, Copy, Clone)]
pub(crate) struct SparseEntity {
    pub archetype_id: ArchetypeId,
    pub generation: u32,
    pub row: u32,
}

pub struct Entities {
    allocator: EntityAllocator,
    live: Vec<Entity>,
    sparse: Vec<Option<SparseEntity>>,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            allocator: EntityAllocator::new(),
            live: Vec::new(),
            sparse: vec![None], // reserved 0 for null entity
        }
    }

    pub(crate) fn alloc(&mut self) -> Entity {
        self.allocator.alloc()
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
