use crate::system::{SysParam, SystemDependencies};
use crate::world::World;
use erased_vec::ErasedVec;
use rustc_hash::FxHashMap;
use std::any::TypeId;
use std::ops::Deref;

pub(crate) type EventId = u32;
pub(crate) type WriterId = u32;
pub trait Event: Sized + 'static {}

pub struct EventState {
    event_id: EventId,
    writer_id: WriterId,
}

pub struct EventW<E: Event>(*mut Vec<E>);

impl<E: Event> EventW<E> {
    pub fn push(&mut self, event: E) {
        unsafe { (*self.0).push(event) }
    }
    pub fn extend(&mut self, events: impl IntoIterator<Item = E>) {
        unsafe { (*self.0).extend(events) }
    }
}

impl<E: Event> SysParam for EventW<E> {
    type Item<'a> = EventW<E>;
    type State = EventState;

    fn init(ecs: &mut World) -> Self::State {
        EventState {
            event_id: ecs.events.id_of::<E>(),
            writer_id: ecs.events.add_writer::<E>(),
        }
    }
    fn add_dependencies(_sys_deps: &mut SystemDependencies, _state: &mut Self::State) {}
    fn fetch<'a>(ecs: *mut World, state: &mut Self::State) -> Self::Item<'a> {
        unsafe { (*ecs).events.writer::<E>(state.event_id, state.writer_id) }
    }
}

pub struct EventR<E: Event>(*const [E]);

impl<E: Event> Deref for EventR<E> {
    type Target = [E];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<E: Event> SysParam for EventR<E> {
    type Item<'a> = EventR<E>;
    type State = EventId;

    fn init(ecs: &mut World) -> Self::State {
        ecs.events.id_of::<E>()
    }
    fn add_dependencies(_sys_deps: &mut SystemDependencies, _state: &mut Self::State) {}
    fn fetch<'a>(ecs: *mut World, state: &mut Self::State) -> Self::Item<'a> {
        unsafe { (*ecs).events.reader::<E>(*state) }
    }
}

struct EventSlot {
    read: ErasedVec,
    writes: Vec<ErasedVec>,
    flush: fn(&mut ErasedVec, &mut [ErasedVec]),
}

pub(crate) struct Events {
    ids: FxHashMap<TypeId, EventId>,
    slots: Vec<EventSlot>,
    built: bool,
}

impl Events {
    pub(crate) fn new() -> Self {
        Self {
            ids: FxHashMap::default(),
            slots: Vec::new(),
            built: false,
        }
    }

    pub(crate) fn build(&mut self) {
        assert!(!self.built, "Attempted to build events twice");
        self.built = true;
    }

    pub(crate) fn register<E: Event>(&mut self) {
        assert!(!self.built, "Attempted to register an event after build");
        let index = self.slots.len();
        let None = self.ids.insert(TypeId::of::<E>(), index as EventId) else {
            panic!("Event {:?} already registered", std::any::type_name::<E>());
        };
        self.slots.push(EventSlot {
            read: ErasedVec::from(Vec::<E>::new()),
            writes: Vec::new(),
            flush: flush_slot::<E>,
        });
    }

    pub(crate) fn add_writer<E: Event>(&mut self) -> WriterId {
        assert!(!self.built, "Attempted to add an event writer after build");
        let event_id = self.id_of::<E>();
        let writes = &mut self.slots[event_id as usize].writes;
        let writer_id = writes.len() as WriterId;
        writes.push(ErasedVec::from(Vec::<E>::new()));
        writer_id
    }

    pub(crate) fn writer<E: Event>(&mut self, event_id: EventId, writer_id: WriterId) -> EventW<E> {
        let writes = &mut self.slots[event_id as usize].writes;
        EventW(writes[writer_id as usize].get_mut())
    }

    pub(crate) fn reader<E: Event>(&self, event_id: EventId) -> EventR<E> {
        let slice: &[E] = self.slots[event_id as usize].read.get();
        EventR(slice as *const [E])
    }

    pub(crate) fn flush(&mut self) {
        for slot in &mut self.slots {
            let flush = slot.flush;
            flush(&mut slot.read, &mut slot.writes);
        }
    }

    pub(crate) fn id_of<E: Event>(&self) -> EventId {
        *self.ids.get(&TypeId::of::<E>()).unwrap()
    }
}

fn flush_slot<E: Event>(read: &mut ErasedVec, writes: &mut [ErasedVec]) {
    match writes {
        [] => read.clear(),
        [writer] => {
            std::mem::swap(read.get_mut::<E>(), writer.get_mut::<E>());
            writer.clear();
        }
        writers => {
            let total = writers.iter().map(|writer| writer.get::<E>().len()).sum();
            let read = read.get_mut::<E>();
            read.clear();
            read.reserve(total);
            for writer in writers {
                read.append(writer.get_mut::<E>());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Ping(u32);
    impl Event for Ping {}

    struct Pong(u8);
    impl Event for Pong {}

    struct DropPing(Arc<AtomicUsize>);
    impl Event for DropPing {}
    impl Drop for DropPing {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn values(events: &Events, id: EventId) -> Vec<u32> {
        events.reader::<Ping>(id).iter().map(|ping| ping.0).collect()
    }

    #[test]
    fn writer_ids_are_per_event() {
        let mut events = Events::new();
        events.register::<Ping>();
        events.register::<Pong>();

        assert_eq!(events.add_writer::<Ping>(), 0);
        assert_eq!(events.add_writer::<Pong>(), 0);
        assert_eq!(events.add_writer::<Ping>(), 1);
    }

    #[test]
    fn one_writer_publishes_then_clears() {
        let mut events = Events::new();
        events.register::<Ping>();
        let id = events.id_of::<Ping>();
        let writer_id = events.add_writer::<Ping>();

        events.writer::<Ping>(id, writer_id).push(Ping(1));
        events.writer::<Ping>(id, writer_id).push(Ping(2));
        events.flush();
        assert_eq!(values(&events, id), vec![1, 2]);

        events.flush();
        assert!(values(&events, id).is_empty());
    }

    #[test]
    fn many_writers_merge_in_registration_order() {
        let mut events = Events::new();
        events.register::<Ping>();
        let id = events.id_of::<Ping>();
        let first = events.add_writer::<Ping>();
        let second = events.add_writer::<Ping>();

        events.writer::<Ping>(id, second).push(Ping(3));
        events.writer::<Ping>(id, first).extend([Ping(1), Ping(2)]);
        events.flush();
        assert_eq!(values(&events, id), vec![1, 2, 3]);

        events.writer::<Ping>(id, first).push(Ping(4));
        events.flush();
        assert_eq!(values(&events, id), vec![4]);
    }

    #[test]
    fn flush_keeps_event_types_apart() {
        let mut events = Events::new();
        events.register::<Ping>();
        events.register::<Pong>();
        let ping_id = events.id_of::<Ping>();
        let pong_id = events.id_of::<Pong>();
        let ping_writer = events.add_writer::<Ping>();
        let pong_writer = events.add_writer::<Pong>();

        events.writer::<Ping>(ping_id, ping_writer).push(Ping(9));
        events.writer::<Pong>(pong_id, pong_writer).push(Pong(4));
        events.flush();

        assert_eq!(values(&events, ping_id), vec![9]);
        assert_eq!(
            events
                .reader::<Pong>(pong_id)
                .iter()
                .map(|pong| pong.0)
                .collect::<Vec<_>>(),
            vec![4]
        );
    }

    #[test]
    fn flush_drops_previous_read_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut events = Events::new();
        events.register::<DropPing>();
        let id = events.id_of::<DropPing>();
        let writer_id = events.add_writer::<DropPing>();

        events
            .writer::<DropPing>(id, writer_id)
            .push(DropPing(Arc::clone(&drops)));
        events.flush();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(events.reader::<DropPing>(id).len(), 1);

        events.flush();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(events.reader::<DropPing>(id).is_empty());
    }

    #[test]
    fn registered_event_with_no_writer_clears() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut events = Events::new();
        events.register::<DropPing>();
        let id = events.id_of::<DropPing>();
        events.slots[id as usize]
            .read
            .get_mut::<DropPing>()
            .push(DropPing(Arc::clone(&drops)));

        events.flush();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(events.reader::<DropPing>(id).is_empty());
    }
}
