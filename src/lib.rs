pub use ecs_macros::Component;

pub mod component;
mod world;
mod archetype;

#[derive(Component, Debug)]
struct CompA;
#[derive(Component, Debug)]
struct CompB;
#[derive(Component, Debug)]
struct CompC;
#[derive(Component, Debug)]
struct CompD;

pub fn test() {
    register_component!(CompA);
    register_component!(CompB);
    register_component!(CompC);
    register_component!(CompD);

    component::build_registry();

    let key_a = archetype_key!(CompA);
    let key_b = archetype_key!(CompA, CompB, CompC);

    println!("keys:\nkey_a: {:?}\nkey_b: {:?}", key_a, key_b);

    let mut world = world::Ecs::new();

    let ent_1 = world.spawn();
    let ent_2 = world.spawn();
    let ent_3 = world.spawn();

    println!("ent_1: {}, ent_2: {}, ent_3: {}", ent_1, ent_2, ent_3);

    world.debug();

    world.add_component(ent_1, CompA);
    world.add_component(ent_1, CompB);
    // world.add_component(ent_1, CompC);

    world.debug();

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
