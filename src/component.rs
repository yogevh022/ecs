use blobvec::BlobVecMeta;
use rustc_hash::FxHashMap;
use std::any::{TypeId, type_name};
use std::mem::MaybeUninit;

pub(crate) const ARCHETYPE_KEY_WORD_BITS: usize = usize::BITS as usize;
pub(crate) const ARCHETYPE_KEY_WORDS: usize = 4;

pub type ComponentId = u32;

pub trait Component: Sized + Clone + 'static {}

pub struct Components {
    ids: FxHashMap<TypeId, ComponentId>,
    storage: Vec<BlobVecMeta>,
    next_id: ComponentId,
    built: bool,
}

impl Components {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ids: FxHashMap::default(),
            storage: vec![unsafe { MaybeUninit::uninit().assume_init() }], // 0 item is null
            built: false,
        }
    }

    pub fn build(&mut self) {
        assert!(!self.built, "Attempted to build components twice");
        self.built = true;
    }

    pub(crate) fn register<T: Component>(&mut self) {
        assert!(!self.built, "Attempted to register a component after build");
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
        &self.storage[id as usize]
    }

    // --- private ---
    fn next_id(&mut self) -> ComponentId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
