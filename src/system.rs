use crate::archetype::ArchetypeKey;
use crate::world::World;
use ecs_macros::impl_into_system_variadic_up_to;
use std::ops::BitOrAssign;

impl_into_system_variadic_up_to!(16);

pub type SystemFn = Box<dyn FnMut(*mut World) + Send>;

pub(crate) struct SystemDependencies {
    components: ArchetypeKey,
}

impl SystemDependencies {
    pub fn new() -> Self {
        Self {
            components: ArchetypeKey::EMPTY,
        }
    }

    pub(crate) fn components(&self) -> ArchetypeKey {
        self.components
    }

    pub(crate) fn add_components(&mut self, key: ArchetypeKey) {
        self.components |= key;
    }
}

impl BitOrAssign for SystemDependencies {
    fn bitor_assign(&mut self, rhs: Self) {
        self.components |= rhs.components;
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
