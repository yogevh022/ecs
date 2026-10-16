use std::any::{Any, TypeId};
use rustc_hash::FxHashMap;

pub struct Resources {
    storage: Vec<Box<dyn Any>>,
    ids: FxHashMap<TypeId, usize>,
}

impl Resources {
    pub fn add_resource<T: Any>(&mut self, resource: T) {
        let id = TypeId::of::<T>();
        let index = self.storage.len();
        self.storage.push(Box::new(resource));
        self.ids.insert(id, index);
    }
}