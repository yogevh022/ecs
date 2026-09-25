use ecs_macros::Component;
use query::{With, Without};
use std::sync::atomic::{AtomicUsize, Ordering};
mod archetype;
pub mod component;
mod ecs;
mod entity;
mod query;
mod event;
mod system;

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

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
#[derive(Component)]
struct CompDrop;
impl Drop for CompDrop {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(never)]
fn test_register() {
    register_component!(CompA);
    register_component!(CompB);
    register_component!(CompC);
    register_component!(CompD);
    register_component!(CompE);
    register_component!(CompF);
    register_component!(CompDrop);
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

fn assert_count(label: &str, actual: usize, expected: usize) {
    assert_eq!(actual, expected, "{label}: got {actual}, expected {expected}");
}

#[inline(never)]
fn test_query(world: &mut ecs::Ecs) {
    // One spawn batch: A 1000, B 1000, A+B 2000, C+D 2000, A+C+D 1000,
    // A+B+C+D 1000, E 500, E+F 500, A+E+F 1000, B+E 500, A+B+E+F 500, C+D+E+F 500.

    let mut count_a = 0;
    let mut sum_a = 0.0f32;
    for (a,) in world.query::<(CompA,)>() {
        sum_a += a.value;
        count_a += 1;
    }
    let mut count_ab = 0;
    let mut sum_ab_a = 0.0f32;
    let mut sum_ab_bx = 0.0f32;
    for (a, b) in world.query::<(CompA, CompB)>() {
        sum_ab_a += a.value;
        sum_ab_bx += b.x;
        count_ab += 1;
    }
    let count_ba = world.query::<(CompB, CompA)>().count();
    let count_cd = world.query::<(CompC, CompD)>().count();
    let count_dc = world.query::<(CompD, CompC)>().count();
    let count_ef = world.query::<(CompE, CompF)>().count();

    assert_count("query<(A,)>", count_a, 6500);
    assert_eq!(sum_a, 4_121_750.0, "query<(A,)> checksum: got {sum_a}");
    assert_count("query<(A, B)>", count_ab, 3500);
    assert_eq!(sum_ab_a, 2_623_250.0, "query<(A, B)> A checksum: got {sum_ab_a}");
    assert_eq!(sum_ab_bx, 2_499_000.0, "query<(A, B)> B.x checksum: got {sum_ab_bx}");
    assert_count("query<(B, A)> same archetypes as (A, B)", count_ba, count_ab);
    assert_count("query<(C, D)>", count_cd, 4500);
    assert_count("query<(D, C)> same archetypes as (C, D)", count_dc, count_cd);
    assert_count("query<(E, F)>", count_ef, 2500);

    // Tuple order is fetch order, not bitmap order. A+B+E+F stores B as (1.0, 2.0)
    // and A.value as i — swapped columns would fail these field checks.
    let mut ab_ef = 0;
    for (a, b) in world.query_filtered::<(CompA, CompB), (With<CompE>, With<CompF>)>() {
        assert_eq!(b.x, 1.0, "(A, B) first should be CompA, second CompB.x");
        assert_eq!(b.y, 2.0, "(A, B) second should be CompB.y");
        assert!(a.value >= 0.0 && a.value < 500.0);
        ab_ef += 1;
    }
    assert_count("filtered (A, B) With<E>+With<F>", ab_ef, 500);

    let mut ba_ef = 0;
    for (b, a) in world.query_filtered::<(CompB, CompA), (With<CompE>, With<CompF>)>() {
        assert_eq!(b.x, 1.0, "(B, A) first should be CompB.x");
        assert_eq!(b.y, 2.0, "(B, A) first should be CompB.y");
        assert!(a.value >= 0.0 && a.value < 500.0);
        ba_ef += 1;
    }
    assert_count("filtered (B, A) With<E>+With<F>", ba_ef, ab_ef);

    assert_count(
        "A Without<B>",
        world.query_filtered::<(CompA,), Without<CompB>>().count(),
        3000,
    );
    assert_count(
        "E Without<F>",
        world.query_filtered::<(CompE,), Without<CompF>>().count(),
        1000,
    );
    assert_count(
        "(A, E) Without<B>",
        world.query_filtered::<(CompA, CompE), Without<CompB>>().count(),
        1000,
    );
    assert_count(
        "D Without<C> (no such archetype)",
        world.query_filtered::<(CompD,), Without<CompC>>().count(),
        0,
    );
    assert_count(
        "A With<C>",
        world.query_filtered::<(CompA,), With<CompC>>().count(),
        2000,
    );
    assert_count(
        "(A, E) With<F> Without<B>",
        world
            .query_filtered::<(CompA, CompE), (With<CompF>, Without<CompB>)>()
            .count(),
        1000,
    );

    print!(
        "{} matches, ",
        count_a + count_ab + count_ba + count_cd + count_dc + count_ef + ab_ef + ba_ef
    );
}

fn sorted_a(world: &mut ecs::Ecs) -> Vec<f32> {
    let mut values: Vec<f32> = world.query::<(CompA,)>().map(|(a,)| a.value).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values
}

fn sorted_ab(world: &mut ecs::Ecs) -> Vec<(f32, f32, f32)> {
    let mut values: Vec<_> = world
        .query::<(CompA, CompB)>()
        .map(|(a, b)| (a.value, b.x, b.y))
        .collect();
    values.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    values
}

fn test_add_remove_correctness() {
    // overwrite stays in the same archetype
    let mut world = ecs::Ecs::new();
    let e = world.new_entity().with(CompA { value: 1.0 }).spawn();
    world.add_component(e, CompA { value: 9.0 });
    assert_eq!(sorted_a(&mut world), vec![9.0]);
    assert_count(
        "overwrite does not add a column",
        world.query::<(CompA, CompB)>().count(),
        0,
    );

    // add to the middle row; source hole must be filled by the last entity
    let mut world = ecs::Ecs::new();
    let e0 = world.new_entity().with(CompA { value: 0.0 }).spawn();
    let e1 = world.new_entity().with(CompA { value: 1.0 }).spawn();
    let e2 = world.new_entity().with(CompA { value: 2.0 }).spawn();
    world.add_component(e1, CompB { x: 10.0, y: 20.0 });
    assert_eq!(sorted_a(&mut world), vec![0.0, 1.0, 2.0]);
    assert_eq!(sorted_ab(&mut world), vec![(1.0, 10.0, 20.0)]);
    let mut without_b: Vec<f32> = world
        .query_filtered::<(CompA,), Without<CompB>>()
        .map(|(a,)| a.value)
        .collect();
    without_b.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(without_b, vec![0.0, 2.0]);

    // dest archetype already exists for the next two adds
    world.add_component(e0, CompB { x: 0.0, y: 1.0 });
    world.add_component(e2, CompB { x: 2.0, y: 3.0 });
    assert_eq!(
        sorted_ab(&mut world),
        vec![(0.0, 0.0, 1.0), (1.0, 10.0, 20.0), (2.0, 2.0, 3.0)]
    );

    // remove from the middle row; remaining A+B pairs must stay aligned
    let mut world = ecs::Ecs::new();
    let e0 = world
        .new_entity()
        .with(CompA { value: 0.0 })
        .with(CompB { x: 0.0, y: 0.0 })
        .spawn();
    let e1 = world
        .new_entity()
        .with(CompA { value: 1.0 })
        .with(CompB { x: 1.0, y: 1.0 })
        .spawn();
    let e2 = world
        .new_entity()
        .with(CompA { value: 2.0 })
        .with(CompB { x: 2.0, y: 2.0 })
        .spawn();
    world.remove_component::<CompB>(e1);
    for (a, b) in world.query::<(CompA, CompB)>() {
        assert_eq!(a.value, b.x, "A/B columns desynced after middle remove");
        assert_eq!(b.x, b.y);
    }
    assert_eq!(
        sorted_ab(&mut world),
        vec![(0.0, 0.0, 0.0), (2.0, 2.0, 2.0)]
    );
    let mut only_a: Vec<f32> = world
        .query_filtered::<(CompA,), Without<CompB>>()
        .map(|(a,)| a.value)
        .collect();
    only_a.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(only_a, vec![1.0]);
    assert_eq!(sorted_a(&mut world), vec![0.0, 1.0, 2.0]);

    world.remove_component::<CompB>(e2);
    assert_eq!(sorted_ab(&mut world), vec![(0.0, 0.0, 0.0)]);

    // missing component is a no-op
    world.remove_component::<CompB>(e1);
    world.remove_component::<CompF>(e0);
    assert_eq!(sorted_ab(&mut world), vec![(0.0, 0.0, 0.0)]);
    assert_eq!(sorted_a(&mut world), vec![0.0, 1.0, 2.0]);

    DROP_COUNT.store(0, Ordering::Relaxed);
    {
        let mut world = ecs::Ecs::new();
        let d0 = world
            .new_entity()
            .with(CompDrop)
            .with(CompA { value: 0.0 })
            .spawn();
        let d1 = world
            .new_entity()
            .with(CompDrop)
            .with(CompA { value: 1.0 })
            .spawn();
        let d2 = world
            .new_entity()
            .with(CompDrop)
            .with(CompA { value: 2.0 })
            .spawn();
        world.remove_component::<CompDrop>(d1);
        assert_eq!(
            DROP_COUNT.load(Ordering::Relaxed),
            1,
            "middle remove must drop T"
        );
        world.add_component(d0, CompDrop);
        assert_eq!(
            DROP_COUNT.load(Ordering::Relaxed),
            2,
            "overwrite must drop old T"
        );
        world.remove_component::<CompDrop>(d2);
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 3);
        drop(world);
    }
    assert_eq!(
        DROP_COUNT.load(Ordering::Relaxed),
        4,
        "world drop must drop remaining CompDrop"
    );
}

#[inline(never)]
fn test_add_remove_batch() {
    const N: usize = 2000;
    let mut world = ecs::Ecs::new();
    let mut entities = Vec::with_capacity(N);
    for i in 0..N {
        entities.push(
            world
                .new_entity()
                .with(CompA { value: i as f32 })
                .spawn(),
        );
    }

    for (i, e) in entities.iter().enumerate() {
        world.add_component(
            *e,
            CompB {
                x: i as f32,
                y: (i * 2) as f32,
            },
        );
    }
    assert_count("batch add B", world.query::<(CompA, CompB)>().count(), N);

    for (i, e) in entities.iter().enumerate() {
        world.add_component(*e, CompA { value: (i + 10_000) as f32 });
    }

    let mut n = 0;
    let mut sum_i = 0.0f32;
    for (a, b) in world.query::<(CompA, CompB)>() {
        let i = a.value - 10_000.0;
        assert_eq!(b.x, i);
        assert_eq!(b.y, i * 2.0);
        sum_i += i;
        n += 1;
    }
    assert_count("batch overwrite A", n, N);
    assert_eq!(sum_i, 1_999_000.0, "batch overwrite checksum: got {sum_i}");

    for (i, e) in entities.iter().enumerate() {
        if i % 2 == 0 {
            world.remove_component::<CompB>(*e);
        }
    }
    assert_count(
        "batch remove even B",
        world.query::<(CompA, CompB)>().count(),
        N / 2,
    );
    for (a, b) in world.query::<(CompA, CompB)>() {
        let i = (a.value - 10_000.0) as i32;
        assert_eq!(i % 2, 1, "even rows still have B after even-remove");
        assert_eq!(b.x, i as f32);
        assert_eq!(b.y, (i * 2) as f32);
    }

    for e in &entities {
        world.remove_component::<CompB>(*e);
    }
    assert_count("batch remove all B", world.query::<(CompA, CompB)>().count(), 0);
    assert_count("batch still all A", world.query::<(CompA,)>().count(), N);

    print!("{} ops, ", N * 4);
}

#[inline(never)]
fn test_add_remove() {
    test_add_remove_correctness();
    test_add_remove_batch();
}

#[inline(never)]
fn test(world: &mut ecs::Ecs) {
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
        component::build();
    });
    print!("done in {:?}\n", q);

    let mut world = ecs::Ecs::new();
    print!("spawning entities... ");
    let q = timed!({
        test_spawn(&mut world);
    });
    print!("done in {:?}\n", q);

    for _ in 0..10 {
        test(&mut world);
    }

    print!("add/remove... ");
    let q = timed!({
        test_add_remove();
    });
    print!("done in {:?}\n", q);
}