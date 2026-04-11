pub use ecs_macros::Component;

mod archetype;
pub mod component;
mod world;

#[derive(Component, Debug)]
struct CompA;
#[derive(Component, Debug)]
struct CompB;
#[derive(Component, Debug)]
struct CompC;
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

    let mut world = world::Ecs::new();
    let e1 = world
        .new_entity()
        .with(CompC)
        .with(CompD { name: "techno" })
        .spawn();

    let e2 = world
        .new_entity()
        .with(CompC)
        .with(CompD { name: "china" })
        .spawn();

    let e3 = world.new_entity().with(CompD { name: "flopper" }).spawn();
    let e4 = world.new_entity().with(CompD { name: "richard" }).spawn();

    world.debug();

    // let key_a = archetype_key!(CompA);
    // let key_b = archetype_key!(CompA, CompB, CompC);

    // let mut world = world::Ecs::new();
    //
    // let ent_1 = world.spawn();
    // let ent_2 = world.spawn();
    // let ent_3 = world.spawn();
    //
    // world.debug();
    //
    // let ca = CompA;
    // let cb = CompB;
    //
    // world.add_component(ent_1, ca);
    // world.add_component(ent_1, cb);
    // world.remove_component::<CompA>(ent_1);
    //
    // world.debug();

    // let mut arch = Archetype::new();
    // arch.add_column::<CompA>();
    // arch.add_column::<CompB>();
    // arch.add_column::<CompC>();
    // arch.remove_column::<CompB>();
    //
    // let col_a = arch.column_of::<CompA>();
    //
    // println!("{:?}", col_a);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        test();
    }
}
