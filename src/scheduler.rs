use crate::archetype::ArchetypeKey;
use crate::event::{Event, EventR};
use crate::query::{Query, With, Without};
use crate::system::{IntoExclusiveSystem, IntoSystem, SystemDependencies, SystemFn};
use crate::world::World;
use ecs_macros::{Component, Event};
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefMutIterator;

// be careful with this.
#[derive(Clone, Copy)]
struct WorldPtr(*mut World);
unsafe impl Send for WorldPtr {}
unsafe impl Sync for WorldPtr {}

impl WorldPtr {
    fn get(self) -> *mut World {
        self.0
    }
}

struct DispatchGroup {
    pub(crate) dependencies: SystemDependencies,
    systems: Vec<SystemFn>,
}

impl DispatchGroup {
    fn disjoint_dependencies(&self, deps: &SystemDependencies) -> bool {
        self.dependencies.components() & deps.components() == ArchetypeKey::EMPTY
            && self
                .dependencies
                .event_writes()
                .iter()
                .all(|id| !deps.event_writes().contains(id))
    }
}

pub struct Scheduler {
    world_id: usize,
    dispatch_groups: Vec<DispatchGroup>,
    dispatch_exclusive: Vec<SystemFn>,
}

impl Scheduler {
    pub fn new(world: &World) -> Self {
        Self {
            world_id: world.id(),
            dispatch_groups: Vec::new(),
            dispatch_exclusive: Vec::new(),
        }
    }

    pub fn add_system<M, F: IntoSystem<M>>(&mut self, world: &mut World, system: F) {
        self.debug_assert_bound(world);
        let (sys_fn, sys_deps) = system.into_system(world);
        self.assign_to_dispatch_group(sys_fn, sys_deps);
    }

    pub fn add_exclusive_system<F: IntoExclusiveSystem>(&mut self, world: &mut World, system: F) {
        self.debug_assert_bound(world);
        self.dispatch_exclusive.push(system.into_exclusive_system());
    }

    fn debug_assert_bound(&self, world: &World) {
        debug_assert_ne!(
            self.world_id,
            world.id(),
            "must be called on the bound World"
        );
    }

    fn assign_to_dispatch_group(&mut self, sys_fn: SystemFn, deps: SystemDependencies) {
        for group in self.dispatch_groups.iter_mut() {
            if group.disjoint_dependencies(&deps) {
                group.dependencies |= deps;
                group.systems.push(sys_fn);
                return;
            }
        }
        self.dispatch_groups.push(DispatchGroup {
            dependencies: deps,
            systems: vec![sys_fn],
        });
    }

    fn run_dispatch_groups(&mut self, world: &mut World) {
        let world = WorldPtr(world as *mut World);
        for group in self.dispatch_groups.iter_mut() {
            group.systems.par_iter_mut().for_each(move |sys_fn| {
                sys_fn(world.get());
            });
        }
    }

    fn run_dispatch_exclusive(&mut self, world: &mut World) {
        for sys_fn in self.dispatch_exclusive.iter_mut() {
            sys_fn(world);
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

fn test_exc(world: *mut World) {}

fn test() {
    let mut world = World::new();
    let mut scheduler = Scheduler::new(&world);

    scheduler.add_system(&mut world, test2);
    scheduler.add_exclusive_system(&mut world, test_exc);
}
