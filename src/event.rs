use erased_vec::ErasedVec;
use rustc_hash::FxHashMap;
use std::any::TypeId;
use std::ops::Deref;
use crate::ecs::Ecs;
use crate::system::SysParam;

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
    type State = EventId;

    fn init(ecs: &mut Ecs) -> Self::State {
        ecs.events.id_of::<E>()
    }
    fn fetch<'a>(ecs: *mut Ecs, state: &mut Self::State) -> Self::Item<'a> {
        unsafe {
            (*ecs).events.writer_id::<E>(*state)
        }
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

    fn init(ecs: &mut Ecs) -> Self::State {
        ecs.events.id_of::<E>()
    }
    fn fetch<'a>(ecs: *mut Ecs, state: &mut Self::State) -> Self::Item<'a> {
        unsafe {
            (*ecs).events.reader_id::<E>(*state)
        }
    }
}

pub type EventId = usize;
pub trait Event: Sized + 'static {
    fn event_id() -> EventId;
}

pub struct Events {
    ids: FxHashMap<TypeId, EventId>,
    read: Vec<ErasedVec>,
    write: Vec<ErasedVec>,
}

impl Events {
    pub(crate) fn new() -> Self {
        Self {
            ids: FxHashMap::default(),
            read: Vec::new(),
            write: Vec::new(),
        }
    }

    pub(crate) fn register<E: Event>(&mut self) {
        let index = self.read.len();
        let None = self.ids.insert(TypeId::of::<E>(), index) else {
            panic!("Event {:?} already registered", std::any::type_name::<E>());
        };
        self.read.push(ErasedVec::from(Vec::<E>::new()));
        self.write.push(ErasedVec::from(Vec::<E>::new()));
    }

    pub(crate) fn swap_read_write(&mut self) {
        std::mem::swap(&mut self.read, &mut self.write);
        for ev in self.write.iter_mut() {
            ev.clear();
        }
    }

    pub(crate) fn reader<E: Event>(&self) -> EventR<E> {
        self.reader_id(E::event_id())
    }

    pub(crate) fn reader_id<E: Event>(&self, id: EventId) -> EventR<E> {
        let slice: &[E] = self.read[id].get();
        EventR(slice as *const [E])
    }

    pub(crate) fn writer<E: Event>(&mut self) -> EventW<E> {
        self.writer_id(E::event_id())
    }

    pub(crate) fn writer_id<E: Event>(&mut self, id: EventId) -> EventW<E> {
        let vec: &mut Vec<E> = self.write[id].get_mut();
        EventW(vec as *mut Vec<E>)
    }

    pub(crate) fn id_of<E: Event>(&self) -> EventId {
        *self.ids.get(&TypeId::of::<E>()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Hit(i32);
    #[derive(Debug, PartialEq)]
    struct Died(u32);

    impl Event for Hit {
        fn event_id() -> EventId {
            0
        }
    }
    impl Event for Died {
        fn event_id() -> EventId {
            1
        }
    }

    fn registered() -> Events {
        let mut events = Events::new();
        events.register::<Hit>();
        events.register::<Died>();
        events
    }

    #[test]
    fn register_assigns_dense_ids_in_order() {
        let events = registered();
        assert_eq!(events.id_of::<Hit>(), 0);
        assert_eq!(events.id_of::<Died>(), 1);
        assert_eq!(events.id_of::<Hit>(), Hit::event_id());
        assert_eq!(events.id_of::<Died>(), Died::event_id());
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn register_twice_panics() {
        let mut events = Events::new();
        events.register::<Hit>();
        events.register::<Hit>();
    }

    #[test]
    fn writes_stay_hidden_until_swap() {
        let mut events = registered();
        let id = events.id_of::<Hit>();
        events.writer_id::<Hit>(id).push(Hit(1));
        assert!(events.reader_id::<Hit>(id).is_empty());

        events.swap_read_write();
        assert_eq!(&*events.reader_id::<Hit>(id), &[Hit(1)]);
        assert!(events.reader::<Hit>().iter().eq([Hit(1)].iter()));
    }

    #[test]
    fn swap_clears_the_write_buffer() {
        let mut events = registered();
        let id = events.id_of::<Hit>();
        events.writer::<Hit>().push(Hit(1));
        events.swap_read_write();

        events.writer_id::<Hit>(id).extend([Hit(2), Hit(3)]);
        events.swap_read_write();
        assert_eq!(&*events.reader_id::<Hit>(id), &[Hit(2), Hit(3)]);
    }

    #[test]
    fn idle_frame_does_not_republish_old_events() {
        let mut events = registered();
        let id = events.id_of::<Hit>();
        events.writer_id::<Hit>(id).push(Hit(1));
        events.swap_read_write();
        assert_eq!(events.reader_id::<Hit>(id).len(), 1);

        events.swap_read_write();
        assert!(events.reader_id::<Hit>(id).is_empty());
    }

    #[test]
    fn event_types_do_not_share_a_queue() {
        let mut events = registered();
        let hits = events.id_of::<Hit>();
        let deaths = events.id_of::<Died>();
        {
            let mut hit_w = events.writer_id::<Hit>(hits);
            let mut died_w = events.writer_id::<Died>(deaths);
            hit_w.push(Hit(4));
            died_w.extend([Died(8), Died(9)]);
        }
        events.swap_read_write();
        assert_eq!(&*events.reader_id::<Hit>(hits), &[Hit(4)]);
        assert_eq!(&*events.reader_id::<Died>(deaths), &[Died(8), Died(9)]);
    }

    #[test]
    fn reader_sees_last_frame_while_writer_fills_this_frame() {
        let mut events = registered();
        let id = events.id_of::<Hit>();
        events.writer_id::<Hit>(id).push(Hit(1));
        events.swap_read_write();

        let read = events.reader_id::<Hit>(id);
        let mut write = events.writer_id::<Hit>(id);
        assert_eq!(&*read, &[Hit(1)]);
        write.push(Hit(2));
        assert_eq!(&*read, &[Hit(1)]);
        drop(write);
        drop(read);

        events.swap_read_write();
        assert_eq!(&*events.reader_id::<Hit>(id), &[Hit(2)]);
    }
}
