use crate::system::{SysParam, SystemDependencies};
use crate::world::World;
use rustc_hash::FxHashMap;
use std::any::{Any, TypeId};
use std::ops::{Deref, DerefMut};

pub type ResourceId = u32;

pub struct ResState {
    id: ResourceId,
    ptr: *mut (),
}

unsafe impl Send for ResState {
    // must be Send because ResState is a state stored inside a SystemFn (which runs on another thread)
    // SAFETY: Resources don't move after building, and scheduler makes sure writing is exclusive
}

pub struct ResW<T: Any>(*mut T);

impl<T: Any> Deref for ResW<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T: Any> DerefMut for ResW<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<T: Any> SysParam for ResW<T> {
    type Item<'a> = ResW<T>;
    type State = ResState;

    fn init(ecs: &mut World) -> Self::State {
        let id = ecs.resources.id_of::<T>().unwrap();
        ResState {
            id,
            ptr: ecs.resources.ptr::<T>(id).cast(),
        }
    }

    fn add_dependencies(sys_deps: &mut SystemDependencies, state: &mut Self::State) {
        sys_deps.add_resource_write(state.id);
    }

    fn fetch<'a>(_ecs: *mut World, state: &mut Self::State) -> Self::Item<'a> {
        ResW(state.ptr.cast())
    }
}

pub struct ResR<T: Any>(*const T);

impl<T: Any> Deref for ResR<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T: Any> SysParam for ResR<T> {
    type Item<'a> = ResR<T>;
    type State = ResState;

    fn init(ecs: &mut World) -> Self::State {
        let id = ecs.resources.id_of::<T>().unwrap();
        ResState {
            id,
            ptr: ecs.resources.ptr::<T>(id).cast(),
        }
    }

    fn add_dependencies(sys_deps: &mut SystemDependencies, state: &mut Self::State) {
        sys_deps.add_resource_read(state.id);
    }

    fn fetch<'a>(_ecs: *mut World, state: &mut Self::State) -> Self::Item<'a> {
        ResR(state.ptr.cast())
    }
}

pub struct Resources {
    storage: Vec<Box<dyn Any>>,
    ids: FxHashMap<TypeId, ResourceId>,
    built: bool,
}

impl Resources {
    pub(crate) fn new() -> Self {
        Self {
            storage: Vec::new(),
            ids: FxHashMap::default(),
            built: false,
        }
    }

    pub(crate) fn build(&mut self) {
        assert!(!self.built, "Attempted to build resources twice");
        self.built = true;
    }

    pub(crate) fn add_resource<T: Any>(&mut self, resource: T) {
        assert!(!self.built, "Attempted to add a resource after build");
        let id = TypeId::of::<T>();
        let None = self.ids.insert(id, self.storage.len() as ResourceId) else {
            panic!("Resource already exists: {:?}", std::any::type_name::<T>())
        };
        self.storage.push(Box::new(resource));
    }

    pub(crate) fn id_of<T: Any>(&self) -> Option<ResourceId> {
        self.ids.get(&TypeId::of::<T>()).copied()
    }

    pub(crate) fn ptr<T: Any>(&mut self, id: ResourceId) -> *mut T {
        let resource = self.storage[id as usize].downcast_mut::<T>().unwrap();
        std::ptr::from_mut(resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::SysParam;

    #[test]
    fn fetch_casts_the_erased_pointer() {
        let mut world = World::new();
        world.resources.add_resource(7u32);
        let mut write = ResW::<u32>::init(&mut world);
        let mut read = ResR::<u32>::init(&mut world);

        let mut write_deps = SystemDependencies::new();
        let mut read_deps = SystemDependencies::new();
        ResW::<u32>::add_dependencies(&mut write_deps, &mut write);
        ResR::<u32>::add_dependencies(&mut read_deps, &mut read);
        assert!(!write_deps.is_disjoint(&read_deps));

        world.resources.add_resource(1u8);
        world.resources.add_resource(2u16);
        let world_ptr = &mut world as *mut World;
        *ResW::<u32>::fetch(world_ptr, &mut write) = 11;
        assert_eq!(*ResR::<u32>::fetch(world_ptr, &mut read), 11);
    }
}
