use std::hint::black_box;
use ecs_macros::Component;
mod archetype;
pub mod component;
mod ecs;
mod query;

macro_rules! timed {
    ($block:block) => {{
        let start = std::time::Instant::now();
        std::hint::black_box($block);
        let end = start.elapsed();
        end
    }};
}

#[derive(Component, Debug)]
struct CompA {
    value: f32,
}
#[derive(Component, Debug)]
struct CompB {
    x: f32,
    y: f32,
}
#[derive(Component, Debug)]
struct CompC {
    rex: &'static str,
}
#[derive(Component, Debug)]
struct CompD {
    name: &'static str,
}
#[derive(Component, Debug)]
struct CompE {
    health: i32,
}
#[derive(Component, Debug)]
struct CompF {
    speed: f32,
}

#[inline(never)]
fn test_register() {
    register_component!(CompA);
    register_component!(CompB);
    register_component!(CompC);
    register_component!(CompD);
    register_component!(CompE);
    register_component!(CompF);
}

#[inline(never)]
fn test_spawn(world: &mut ecs::Ecs) {
    let mut q = 0;
    // Archetype: A only (1000 entities)
    for i in 0..1000 {
        world.new_entity().with(CompA { value: i as f32 }).spawn();
        q += 1;
    }

    // Archetype: B only (1000 entities)
    for i in 0..1000 {
        world
            .new_entity()
            .with(CompB {
                x: i as f32,
                y: (i * 2) as f32,
            })
            .spawn();
        q += 1;
    }

    // Archetype: A + B (2000 entities)
    for i in 0..2000 {
        world
            .new_entity()
            .with(CompA { value: i as f32 })
            .with(CompB {
                x: i as f32,
                y: i as f32,
            })
            .spawn();
        q += 1;
    }

    // Archetype: C + D (2000 entities)
    for i in 0..2000 {
        world
            .new_entity()
            .with(CompC { rex: "warrior" })
            .with(CompD { name: "unit" })
            .spawn();
        q += 1;
    }

    // Archetype: A + C + D (1000 entities)
    for i in 0..1000 {
        world
            .new_entity()
            .with(CompA { value: i as f32 })
            .with(CompC { rex: "mage" })
            .with(CompD { name: "named_mage" })
            .spawn();
        q += 1;
    }

    // Archetype: A + B + C + D (1000 entities)
    for i in 0..1000 {
        world
            .new_entity()
            .with(CompA { value: i as f32 })
            .with(CompB {
                x: i as f32,
                y: 0.0,
            })
            .with(CompC { rex: "rogue" })
            .with(CompD { name: "shadow" })
            .spawn();
        q += 1;
    }

    // Archetype: E only (500 entities)
    for i in 0..500 {
        world.new_entity().with(CompE { health: i * 10 }).spawn();
        q += 1;
    }

    // Archetype: E + F (500 entities)
    for i in 0..500 {
        world
            .new_entity()
            .with(CompE { health: i * 5 })
            .with(CompF {
                speed: i as f32 * 1.5,
            })
            .spawn();
        q += 1;
    }

    // Archetype: A + E + F (1000 entities)
    for i in 0..1000 {
        world
            .new_entity()
            .with(CompA { value: i as f32 })
            .with(CompE { health: 100 })
            .with(CompF { speed: 3.0 })
            .spawn();
        q += 1;
    }

    // Archetype: B + E (500 entities)
    for i in 0..500 {
        world
            .new_entity()
            .with(CompB {
                x: i as f32,
                y: i as f32,
            })
            .with(CompE { health: 50 })
            .spawn();
        q += 1;
    }

    // Archetype: A + B + E + F (500 entities)
    for i in 0..500 {
        world
            .new_entity()
            .with(CompA { value: i as f32 })
            .with(CompB { x: 1.0, y: 2.0 })
            .with(CompE { health: 200 })
            .with(CompF { speed: 10.0 })
            .spawn();
        q += 1;
    }

    // Archetype: C + D + E + F (500 entities)
    for i in 0..500 {
        world
            .new_entity()
            .with(CompC { rex: "paladin" })
            .with(CompD { name: "holy" })
            .with(CompE { health: 300 })
            .with(CompF { speed: 2.5 })
            .spawn();
        q += 1;
    }

    print!("{} entities, ", q);
}

#[inline(never)]
fn test_query(world: &mut ecs::Ecs) {
    let mut q = 0;
    for (_a,) in world.query::<(CompA,)>() {
        q += 1;
    }
    for (_a, _b) in world.query::<(CompA, CompB)>() {
        q += 1;

    }
    for (_c, _d) in world.query::<(CompC, CompD)>() {
        q += 1;

    }
    for (_e, _f) in world.query::<(CompE, CompF)>() {
        q += 1;

    }
    for (_d,) in world.query_specific::<(CompD,), (CompC,)>() {
        q += 1;

    }
    for (_a,) in world.query_specific::<(CompA,), (CompB,)>() {
        q += 1;
    }
    for (_e,) in world.query_specific::<(CompE,), (CompF,)>() {
        q += 1;
    }
    for (_a, _e) in world.query_specific::<(CompA, CompE), (CompB,)>() {
        q += 1;
    }
    print!("{} queries, ", q);
}

#[inline(never)]
fn test(world: &mut ecs::Ecs) {
    print!("spawning entities... ");
    let q = timed!({
        test_spawn(world);
    });
    print!("done in {:?}\n", q);

    print!("querying... ");
    let q = timed!({
        test_query(world);
    });
    print!("done in {:?}\n", q);
}

fn main() {
    print!("Registering components... ");
    let q = timed!({
        test_register();
        component::build_registry();
    });
    print!("done in {:?}\n", q);

    let mut world = ecs::Ecs::new();
    for _ in 0..10 {
        test(&mut world);
    }
}