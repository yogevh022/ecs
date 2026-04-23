mod archetype;
mod component;
mod ecs;
mod query;

pub mod prelude {
    pub use crate::component::Component; // component trait
    pub use crate::ecs::{Ecs, Entity, EntityBuilder};
    pub use crate::register_component;
    pub use ecs_macros::Component; // component derive
}
