use crate::ecs::Ecs;
use ecs_macros::impl_into_system_variadic_up_to;

impl_into_system_variadic_up_to!(16);

pub type SystemFn = Box<dyn FnMut(*mut Ecs)>;

pub trait SysParam {
    type Item<'a>;
    type State: 'static;
    fn init(ecs: &mut Ecs) -> Self::State;
    fn fetch<'a>(ecs: *mut Ecs, state: &mut Self::State) -> Self::Item<'a>;
}

pub trait IntoSystem<Marker> {
    fn into_system(self, ecs: &mut Ecs) -> SystemFn;
}