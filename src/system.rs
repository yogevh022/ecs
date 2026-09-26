use crate::world::World;
use ecs_macros::impl_into_system_variadic_up_to;

impl_into_system_variadic_up_to!(16);

pub type SystemFn = Box<dyn FnMut(*mut World)>;

pub trait SysParam {
    type Item<'a>;
    type State: 'static;
    fn init(ecs: &mut World) -> Self::State;
    fn fetch<'a>(ecs: *mut World, state: &mut Self::State) -> Self::Item<'a>;
}

pub trait IntoSystem<Marker> {
    fn into_system(self, ecs: &mut World) -> SystemFn;
}