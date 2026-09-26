use ecs_macros::Component;
use crate::event::{Event, EventId, EventR};
use crate::query::{Query, With, Without};
use crate::system::IntoSystem;

pub struct Scheduler;

impl Scheduler {
    pub fn add_system<M, F: IntoSystem<M>>(&mut self, system: F) {

    }
}

struct TestE;
impl Event for TestE {
    fn event_id() -> EventId {
        todo!()
    }
}

#[derive(Component)]
struct TestC;

#[derive(Component)]
struct TestD;

#[derive(Component)]
struct TestF;

fn test2(q: EventR<TestE>, q2: Query<(TestC,), (With<(TestC, TestD)>, Without<TestF>)>) {

}

fn test() {
    let mut scheduler = Scheduler;
    scheduler.add_system(test2);
}