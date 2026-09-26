use crate::event::{Event, EventR};
use crate::query::{Query, With, Without};
use crate::system::{IntoSystem, SystemFn};
use crate::world::World;
use ecs_macros::{Component, Event};

pub struct Scheduler {
    world_id: Option<usize>,
    systems: Vec<SystemFn>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            world_id: None,
            systems: Vec::new(),
        }
    }

    pub fn add_system<M, F: IntoSystem<M>>(&mut self, world: &mut World, system: F) {
        debug_assert_ne!(self.world_id, Some(world.id()), "add_system mut be called on bound World");
        self.bind(world);
        self.systems.push(system.into_system(world));
    }

    pub fn bind(&mut self, world: &World) {
        match self.world_id {
            Some(_) => panic!("Scheduler already bound"),
            None => self.world_id = Some(world.id()),
        }
    }
}

#[derive(Event)]
struct TestE;

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
