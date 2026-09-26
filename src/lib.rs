mod archetype;
mod component;
mod world;
mod entity;
mod query;
mod event;
mod system;
mod scheduler;

pub mod prelude {
    pub use crate::component::Component; // component trait
    pub use crate::world::{World, EntityBuilder};
    pub use crate::entity::Entity;
    pub use ecs_macros::Component; // component derive
}
