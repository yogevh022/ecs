use blobvec::BlobVecMeta;
use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;
use std::any::{TypeId, type_name};
use std::mem::MaybeUninit;
use std::sync::OnceLock;

pub(crate) const ARCHETYPE_KEY_WORD_BITS: usize = usize::BITS as usize;
pub(crate) const ARCHETYPE_KEY_WORDS: usize = 4;
pub(crate) static COMPONENTS: OnceLock<RwLock<Components>> = OnceLock::new();

pub type ComponentId = usize;

pub trait Component: Sized + 'static {
    fn component_id() -> ComponentId;
}

pub struct Components {
    ids: FxHashMap<TypeId, ComponentId>,
    storage: Vec<BlobVecMeta>,
    id_counter: usize,
    built: bool,
}

impl Components {
    pub fn new() -> Self {
        Self {
            id_counter: 1,
            ids: FxHashMap::default(),
            storage: vec![unsafe { MaybeUninit::uninit().assume_init() }], // 0 item is null
            built: false,
        }
    }

    pub fn build(&mut self) {
        debug_assert!(!self.built, "Attempted to build components twice!");
        self.built = true;
    }

    pub(crate) fn register<T: Component>(&mut self) {
        debug_assert!(
            !self.built,
            "Attempted to register a component after build!"
        );
        let type_id = TypeId::of::<T>();
        let id = self.next_id();
        let None = self.ids.insert(type_id, id) else {
            panic!(
                "Attempted to register component twice: {:?} (component_id: {:?}, type_id: {:?})",
                type_name::<T>(),
                id,
                type_id
            );
        };
        self.storage.push(BlobVecMeta::new::<T>());
    }

    pub(crate) fn id_of<T: Component>(&self) -> ComponentId {
        self.ids[&TypeId::of::<T>()]
    }

    pub(crate) fn storage_meta_of_id(&self, id: ComponentId) -> &BlobVecMeta {
        &self.storage[id]
    }

    // --- private ---
    fn next_id(&mut self) -> ComponentId {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }
}

pub(crate) fn lock() -> &'static RwLock<Components> {
    COMPONENTS.get_or_init(|| RwLock::new(Components::new()))
}

pub fn build() {
    // engine-only interface
    lock().write().build();
}

pub fn get<'a>() -> RwLockReadGuard<'a, Components> {
    lock().read()
}

#[macro_export]
macro_rules! register_component {
    ($T:ty) => {
        $crate::component::lock().write().register::<$T>();
    };
}
