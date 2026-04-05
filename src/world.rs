use std::sync::atomic::{AtomicUsize, Ordering};
use rustc_hash::FxHashMap;
use crate::component::Component;
use crate::component::ArchetypeKey;

pub type Entity = usize;

struct EntityAllocator {
    counter: AtomicUsize,
    free_list: Vec<Entity>,
}

impl EntityAllocator {
    fn new() -> Self {
        Self {
            counter: AtomicUsize::new(1),
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
            id
        } else {
            self.counter.fetch_add(1, Ordering::Relaxed)
        }
    }

    fn free(&mut self, id: Entity) {
        self.free_list.push(id);
    }
}

// struct EntityBuilder<'e> {
//     ecs: &'e mut Ecs,
//     components: Vec<Box<dyn Component>>,
// }
//
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

pub struct Ecs {
    entity_allocator: EntityAllocator,
    entities: Vec<Entity>,
    // archetypes: FxHashMap<ArchetypeKey, Archetype>,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            entity_allocator: EntityAllocator::new(),
            entities: Vec::new(),
            // archetypes: FxHashMap::default(),
        }
    }

    pub fn spawn(&mut self) -> Entity {
        let id = self.entity_allocator.alloc();
        self.entities.push(id);
        id
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.entity_allocator.free(entity);
        self.entities.swap_remove(entity);
    }
}
