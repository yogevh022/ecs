use crate::system::{IntoSystem, SystemDependencies, SystemFn};
use crate::world::World;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefMutIterator;

pub struct Scheduler {
    world_id: usize,
    dispatch_groups: Vec<DispatchGroup>,
}

impl Scheduler {
    pub fn new(world: &World) -> Self {
        Self {
            world_id: world.id(),
            dispatch_groups: Vec::new(),
        }
    }

    pub fn add_system<M, F: IntoSystem<M>>(&mut self, world: &mut World, system: F) {
        assert_eq!(
            self.world_id,
            world.id(),
            "must be called on the bound World"
        );
        assert!(!world.is_built(), "cannot add a system after build");
        let (sys_fn, sys_deps) = system.into_system(world);
        self.assign_to_dispatch_group(sys_fn, sys_deps);
    }

    pub fn tick(&mut self, world: &mut World) {
        assert_eq!(
            self.world_id,
            world.id(),
            "must be called on the bound World"
        );
        assert!(world.is_built(), "world must be built before tick");
        self.run_dispatch_groups(world);
        world.events.flush();
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
}

struct DispatchGroup {
    dependencies: SystemDependencies,
    systems: Vec<SystemFn>,
}

impl DispatchGroup {
    fn disjoint_dependencies(&self, deps: &SystemDependencies) -> bool {
        self.dependencies.is_disjoint(deps)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventR, EventW};
    use crate::resource::{ResR, ResW};
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Arc, Mutex};

    struct Score(i32);
    struct Lives(i32);

    struct Ping(u32);
    impl Event for Ping {}

    #[test]
    fn readers_of_one_resource_share_a_group() {
        let mut world = World::new();
        world.resources.add_resource(Score(3));
        let mut scheduler = Scheduler::new(&world);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&seen);
        let second = Arc::clone(&seen);

        scheduler.add_system(&mut world, move |score: ResR<Score>| {
            first.lock().unwrap().push(score.0);
        });
        scheduler.add_system(&mut world, move |score: ResR<Score>| {
            second.lock().unwrap().push(score.0);
        });

        assert_eq!(scheduler.dispatch_groups.len(), 1);
        assert_eq!(scheduler.dispatch_groups[0].systems.len(), 2);
        world.build();
        scheduler.tick(&mut world);
        let mut values = seen.lock().unwrap().clone();
        values.sort();
        assert_eq!(values, vec![3, 3]);
    }

    #[test]
    fn writer_runs_before_a_later_reader() {
        let mut world = World::new();
        world.resources.add_resource(Score(1));
        let mut scheduler = Scheduler::new(&world);
        let seen = Arc::new(AtomicI32::new(-1));
        let seen_in_system = Arc::clone(&seen);

        scheduler.add_system(&mut world, |mut score: ResW<Score>| {
            score.0 = 9;
        });
        scheduler.add_system(&mut world, move |score: ResR<Score>| {
            seen_in_system.store(score.0, Ordering::SeqCst);
        });

        assert_eq!(scheduler.dispatch_groups.len(), 2);
        world.build();
        scheduler.tick(&mut world);
        assert_eq!(seen.load(Ordering::SeqCst), 9);
        assert_eq!(score_value(&mut world), 9);
    }

    #[test]
    fn reader_scheduled_first_sees_the_old_value() {
        let mut world = World::new();
        world.resources.add_resource(Score(1));
        let mut scheduler = Scheduler::new(&world);
        let seen = Arc::new(AtomicI32::new(-1));
        let seen_in_system = Arc::clone(&seen);

        scheduler.add_system(&mut world, move |score: ResR<Score>| {
            seen_in_system.store(score.0, Ordering::SeqCst);
        });
        scheduler.add_system(&mut world, |mut score: ResW<Score>| {
            score.0 = 9;
        });

        assert_eq!(scheduler.dispatch_groups.len(), 2);
        world.build();
        scheduler.tick(&mut world);
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        assert_eq!(score_value(&mut world), 9);
    }

    #[test]
    fn writers_of_the_same_resource_do_not_share_a_group() {
        let mut world = World::new();
        world.resources.add_resource(Score(0));
        let mut scheduler = Scheduler::new(&world);

        scheduler.add_system(&mut world, |mut score: ResW<Score>| {
            score.0 += 1;
        });
        scheduler.add_system(&mut world, |mut score: ResW<Score>| {
            score.0 += 10;
        });

        assert_eq!(scheduler.dispatch_groups.len(), 2);
        world.build();
        scheduler.tick(&mut world);
        assert_eq!(score_value(&mut world), 11);
    }

    #[test]
    fn different_resources_can_run_in_one_system_and_one_group() {
        let mut world = World::new();
        world.resources.add_resource(Score(1));
        world.resources.add_resource(Lives(4));
        let mut scheduler = Scheduler::new(&world);
        let seen = Arc::new(AtomicI32::new(-1));
        let seen_in_system = Arc::clone(&seen);

        scheduler.add_system(&mut world, |lives: ResR<Lives>, mut score: ResW<Score>| {
            score.0 += lives.0;
        });
        scheduler.add_system(&mut world, move |lives: ResR<Lives>| {
            seen_in_system.store(lives.0, Ordering::SeqCst);
        });

        assert_eq!(scheduler.dispatch_groups.len(), 1);
        assert_eq!(scheduler.dispatch_groups[0].systems.len(), 2);
        world.build();
        scheduler.tick(&mut world);
        assert_eq!(score_value(&mut world), 5);
        assert_eq!(seen.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn event_written_during_a_tick_is_read_on_the_next_tick() {
        let mut world = World::new();
        world.register_event::<Ping>();
        let mut scheduler = Scheduler::new(&world);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_in_system = Arc::clone(&seen);

        scheduler.add_system(&mut world, |mut events: EventW<Ping>| {
            events.push(Ping(4));
        });
        scheduler.add_system(&mut world, move |events: EventR<Ping>| {
            seen_in_system
                .lock()
                .unwrap()
                .push(events.iter().map(|event| event.0).collect::<Vec<_>>());
        });

        world.build();
        scheduler.tick(&mut world);
        scheduler.tick(&mut world);
        assert_eq!(seen.lock().unwrap().as_slice(), [Vec::new(), vec![4]]);
    }

    #[test]
    #[should_panic(expected = "cannot add a system after build")]
    fn add_system_after_build_panics() {
        let mut world = World::new();
        world.resources.add_resource(Score(0));
        let mut scheduler = Scheduler::new(&world);
        world.build();
        scheduler.add_system(&mut world, |_score: ResR<Score>| {});
    }

    #[test]
    #[should_panic(expected = "world must be built before tick")]
    fn tick_before_build_panics() {
        let mut world = World::new();
        let mut scheduler = Scheduler::new(&world);
        scheduler.tick(&mut world);
    }

    fn score_value(world: &mut World) -> i32 {
        let id = world.resources.id_of::<Score>().unwrap();
        unsafe { (*world.resources.ptr::<Score>(id)).0 }
    }
}
