use erased_vec::ErasedVec;
use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;
use std::any::TypeId;


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
    pub fn new() -> Self {
        Self {
            ids: FxHashMap::default(),
            read: Vec::new(),
            write: Vec::new(),
        }
    }

    pub(crate) fn register<E: Event>(&mut self) {
        let index = self.read.len();
        self.ids.insert(TypeId::of::<E>(), index);
        self.read.push(ErasedVec::from(Vec::<E>::new()));
        self.write.push(ErasedVec::from(Vec::<E>::new()));
    }

    pub(crate) fn swap_read_write(&mut self) {
        std::mem::swap(&mut self.read, &mut self.write);
        for ev in self.write.iter_mut() {
            ev.clear();
        }
    }

    pub(crate) fn id_of<E: Event>(&self) -> EventId {
        *self.ids.get(&TypeId::of::<E>()).unwrap()
    }
}
