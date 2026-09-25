mod archetype;
mod component;
mod ecs;
mod entity;
mod query;

pub mod prelude {
    pub use crate::component::Component; // component trait
    pub use crate::ecs::{Ecs, EntityBuilder};
    pub use crate::entity::Entity;
    pub use crate::register_component;
    pub use ecs_macros::Component; // component derive
}
