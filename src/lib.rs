pub use ecs_macros::Component;

mod archetype;
pub mod component;
mod ecs;
mod query;

#[derive(Component, Debug)]
struct CompA;
#[derive(Component, Debug)]
struct CompB;
#[derive(Component, Debug)]
struct CompC {
    rex: &'static str,
}
#[derive(Component, Debug)]
struct CompD {
    name: &'static str,
}

pub fn test() {
    register_component!(CompA);
    register_component!(CompB);
    register_component!(CompC);
    register_component!(CompD);

    component::build_registry();

    let mut world = ecs::Ecs::new();
    let e1 = world
        .new_entity()
        .with(CompC { rex: "blade" })
        .with(CompD { name: "techno" })
        .spawn();

    let e2 = world
        .new_entity()
        .with(CompC { rex: "malice" })
        .with(CompD { name: "china" })
        .spawn();

    let e3 = world.new_entity().with(CompD { name: "flopper" }).spawn();
    let e4 = world.new_entity().with(CompD { name: "richard" }).spawn();

    for (d,) in world.query_specific::<(CompD,), (CompC,)>() {
        dbg!(d);
    }

    for (c, d) in world.query::<(CompC, CompD)>() {
        dbg!(c, d);
    }

    for (_,) in world.query::<(CompA,)>() {
        dbg!("empty");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        test();
    }
}
