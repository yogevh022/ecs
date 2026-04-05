use blobvec::BlobVec;
use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;
use std::any::{TypeId, type_name};
use std::fmt::Debug;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(crate) const ARCHETYPE_KEY_WORD_BITS: usize = usize::BITS as usize;
pub(crate) const ARCHETYPE_KEY_WORDS: usize = 4;
pub(crate) static COMPONENT_REGISTRY: OnceLock<RwLock<ComponentRegistry>> = OnceLock::new();

pub type ComponentId = usize;

pub trait Component: Sized + 'static {
    fn component_id() -> ComponentId;
}

#[derive(PartialEq, Eq, Hash)]
pub struct ArchetypeKey(pub(crate) [usize; ARCHETYPE_KEY_WORDS]);
impl ArchetypeKey {
    pub const EMPTY: ArchetypeKey = ArchetypeKey([0; ARCHETYPE_KEY_WORDS]);
}

impl Debug for ArchetypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArchKey<")?;
        for (i, word) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", word)?;
        }
        write!(f, ">")
    }
}

pub struct Archetype {
    components: Vec<BlobVec>,
    component_ids: Vec<ComponentId>,
}

impl Archetype {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            component_ids: Vec::new(),
        }
    }

    // --- accessors ---
    pub fn get_column<T: Component>(&self) -> &BlobVec {
        match self.get_column_index::<T>() {
            Some(col_idx) => &self.components[col_idx],
            None => panic!("Component {} not found in archetype", type_name::<T>()),
        }
    }

    pub fn get_column_mut<T: Component>(&mut self) -> &mut BlobVec {
        match self.get_column_index::<T>() {
            Some(col_idx) => &mut self.components[col_idx],
            None => panic!("Component {} not found in archetype", type_name::<T>()),
        }
    }

    // --- insertion ---
    pub(crate) fn add_column<T: Component>(&mut self) {
        let column = BlobVec::new::<T>();
        self.components.push(column);
        self.component_ids.push(T::component_id());
    }

    // --- private ---
    #[inline]
    fn get_column_index<T: Component>(&self) -> Option<usize> {
        let comp_id = T::component_id();
        self.component_ids.iter().position(|id| *id == comp_id)
    }
}

pub struct ComponentRegistry {
    registry: FxHashMap<TypeId, usize>,
    id_counter: AtomicUsize,
    built: bool,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            id_counter: AtomicUsize::new(1),
            registry: FxHashMap::default(),
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
        if let Some(id) = self.registry.get(&type_id) {
            panic!(
                "Attempted to register component twice: {:?} (component_id: {:?}, type_id: {:?})",
                type_name::<T>(),
                id,
                type_id
            );
        }
        let id = self.id_counter.fetch_add(1, Ordering::Relaxed);
        self.registry.insert(type_id, id);
    }

    pub(crate) fn component_id_of<T: Component>(&self) -> ComponentId {
        self.registry[&TypeId::of::<T>()]
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
            let mut key = $crate::component::ArchetypeKey::EMPTY;
            $(
                let component_bit = $T::component_id() - 1;
                let bit = component_bit % $crate::component::ARCHETYPE_KEY_WORD_BITS;
                let word = component_bit / $crate::component::ARCHETYPE_KEY_WORDS;
                key.0[word] |= 1 << bit;
            )+
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
