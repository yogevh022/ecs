use blobvec::BlobVecMeta;
use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;
use std::any::{TypeId, type_name};
use std::mem::MaybeUninit;
use std::sync::OnceLock;

pub(crate) const ARCHETYPE_KEY_WORD_BITS: u32 = usize::BITS;
pub(crate) const ARCHETYPE_KEY_WORDS: u32 = 4;
pub(crate) static COMPONENT_REGISTRY: OnceLock<RwLock<ComponentRegistry>> = OnceLock::new();

pub type ComponentId = u32;

pub trait Component: Sized + 'static {
    fn component_id() -> ComponentId;
}

pub struct ComponentRegistry {
    registry: FxHashMap<TypeId, ComponentId>,
    storage_registry: Vec<BlobVecMeta>,
    id_counter: u32,
    built: bool,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            id_counter: 1,
            registry: FxHashMap::default(),
            storage_registry: vec![unsafe { MaybeUninit::uninit().assume_init() }], // 0 item is null
            built: false,
        }
    }

    pub fn build(&mut self) {
        debug_assert!(!self.built, "Attempted to build component registry twice!");
        self.built = true;
    }

    pub(crate) fn register<T: Component>(&mut self) {
        debug_assert!(
            !self.built,
            "Attempted to register component after building component registry!"
        );
        let type_id = TypeId::of::<T>();
        let id = self.next_id();
        let None = self.registry.insert(type_id, id) else {
            panic!(
                "Attempted to register component twice: {:?} (component_id: {:?}, type_id: {:?})",
                type_name::<T>(),
                id,
                type_id
            );
        };
        self.storage_registry.push(BlobVecMeta::new::<T>());
    }

    pub(crate) fn component_id_of<T: Component>(&self) -> ComponentId {
        self.registry[&TypeId::of::<T>()]
    }

    pub(crate) fn storage_meta_of_id(&self, id: ComponentId) -> &BlobVecMeta {
        &self.storage_registry[id as usize]
    }

    // --- private ---
    fn next_id(&mut self) -> ComponentId {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }
}

pub(crate) fn component_registry_lock() -> &'static RwLock<ComponentRegistry> {
    &COMPONENT_REGISTRY.get_or_init(|| RwLock::new(ComponentRegistry::new()))
}

pub fn build_registry() {
    // engine-only interface
    component_registry_lock().write().build();
}
pub fn registry<'a>() -> RwLockReadGuard<'a, ComponentRegistry> {
    component_registry_lock().read()
}

#[macro_export]
macro_rules! archetype_key {
    ($($T:ident),+) => {
        {
            let mut key = $crate::archetype::ArchetypeKey::EMPTY;
            $(key = key.with::<$T>();)+
            key
        }
    };
}

#[macro_export]
macro_rules! register_component {
    ($T:ty) => {
        $crate::component::component_registry_lock()
            .write()
            .register::<$T>();
    };
}
