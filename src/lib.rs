mod archetype;
mod component;
mod entity;
mod event;
mod query;
mod scheduler;
mod system;
mod world;

pub use query::{ComponentGroup, QueryFilter, Queryable};
pub use system::{IntoExclusiveSystem, IntoSystem, SysParam, SystemFn};

pub mod prelude {
    pub use crate::component::Component;
    pub use crate::entity::Entity;
    pub use crate::event::{Event, EventR, EventW};
    pub use crate::query::{Query, With, Without};
    pub use crate::scheduler::Scheduler;
    pub use crate::world::{EntityBuilder, World};
    pub use ecs_macros::{Component, Event};
}
