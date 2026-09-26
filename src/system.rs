use crate::archetype::ArchetypeKey;
use crate::event::EventId;
use crate::world::World;
use ecs_macros::impl_into_system_variadic_up_to;
use std::ops::BitOrAssign;

impl_into_system_variadic_up_to!(16);

pub type SystemFn = Box<dyn FnMut(*mut World) + Send>;

pub(crate) struct SystemDependencies {
    components: ArchetypeKey,
    event_r: Vec<EventId>,
    event_w: Vec<EventId>,
}

impl SystemDependencies {
    pub fn new() -> Self {
        Self {
            components: ArchetypeKey::EMPTY,
            event_r: Vec::new(),
            event_w: Vec::new(),
        }
    }

    pub(crate) fn components(&self) -> ArchetypeKey {
        self.components
    }

    pub(crate) fn event_writes(&self) -> &[EventId] {
        &self.event_w
    }

    pub(crate) fn add_components(&mut self, key: ArchetypeKey) {
        self.components |= key;
    }

    pub(crate) fn add_event_r(&mut self, id: EventId) {
        self.event_r.push(id);
    }

    pub(crate) fn add_event_w(&mut self, id: EventId) {
        self.event_w.push(id);
    }
}

impl BitOrAssign for SystemDependencies {
    fn bitor_assign(&mut self, rhs: Self) {
        self.components |= rhs.components;
        self.event_r.extend(rhs.event_r);
        self.event_w.extend(rhs.event_w);
    }
}

pub trait SysParam {
    type Item<'a>;
    type State: 'static;
    fn init(ecs: &mut World) -> Self::State;
    fn add_dependencies(sys_deps: &mut SystemDependencies, state: &mut Self::State);
    fn fetch<'a>(ecs: *mut World, state: &mut Self::State) -> Self::Item<'a>;
}

pub trait IntoSystem<Marker> {
    fn into_system(self, ecs: &mut World) -> (SystemFn, SystemDependencies);
}

pub trait IntoExclusiveSystem {
    fn into_exclusive_system(self) -> SystemFn;
}

impl<F> IntoExclusiveSystem for F where F: FnMut(*mut World) + Send + 'static {
    fn into_exclusive_system(self) -> SystemFn {
        Box::new(self)
    }
}
