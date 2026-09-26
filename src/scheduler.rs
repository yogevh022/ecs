use crate::event::{Event, EventR};
use crate::query::{Query, With, Without};
use crate::system::{IntoSystem, SystemFn};
use crate::world::World;
use ecs_macros::Component;

pub struct Scheduler {
    systems: Vec<SystemFn>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }
    pub fn add_system<M, F: IntoSystem<M>>(&mut self, world: &mut World, system: F) {
        self.systems.push(system.into_system(world));
    }
}

struct TestE;
impl Event for TestE {}

#[derive(Component)]
struct TestC;

#[derive(Component)]
struct TestD;

#[derive(Component)]
struct TestF;

fn test2(q: EventR<TestE>, q2: Query<(TestC,), (With<(TestC, TestD)>, Without<TestF>)>) {}

fn test() {
    let mut world = World::new();
    let mut scheduler = Scheduler::new();
    scheduler.add_system(&mut world, test2);
}
