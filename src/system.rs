use crate::archetype::ArchetypeKey;
use crate::resource::ResourceId;
use crate::world::World;
use ecs_macros::impl_into_system_variadic_up_to;
use std::ops::BitOrAssign;

impl_into_system_variadic_up_to!(16);

pub type SystemFn = Box<dyn FnMut(*mut World) + Send>;

pub(crate) struct SystemDependencies {
    components: ArchetypeKey,
    resource_reads: Vec<ResourceId>,
    resource_writes: Vec<ResourceId>,
}

impl SystemDependencies {
    pub fn new() -> Self {
        Self {
            components: ArchetypeKey::EMPTY,
            resource_reads: Vec::new(),
            resource_writes: Vec::new(),
        }
    }

    pub(crate) fn add_components(&mut self, key: ArchetypeKey) {
        self.components |= key;
    }

    pub(crate) fn add_resource_read(&mut self, resource: ResourceId) {
        self.resource_reads.push(resource);
    }

    pub(crate) fn add_resource_write(&mut self, resource: ResourceId) {
        self.resource_writes.push(resource);
    }

    pub(crate) fn is_disjoint(&self, other: &SystemDependencies) -> bool {
        self.components & other.components == ArchetypeKey::EMPTY
            && !overlaps(&self.resource_writes, &other.resource_reads)
            && !overlaps(&self.resource_writes, &other.resource_writes)
            && !overlaps(&other.resource_writes, &self.resource_reads)
    }
}

impl BitOrAssign for SystemDependencies {
    fn bitor_assign(&mut self, rhs: Self) {
        self.components |= rhs.components;
        self.resource_reads.extend_from_slice(&rhs.resource_reads);
        self.resource_writes.extend_from_slice(&rhs.resource_writes);
    }
}

fn overlaps(a: &[ResourceId], b: &[ResourceId]) -> bool {
    a.iter().any(|id| b.contains(id))
}

pub trait SysParam {
    type Item<'a>;
    type State: 'static;
    fn init(ecs: &mut World) -> Self::State;
    fn add_dependencies(sys_deps: &mut SystemDependencies, state: &mut Self::State);
    fn fetch<'a>(ecs: *mut World, state: &mut Self::State) -> Self::Item<'a>;
}

pub trait IntoSystem<Marker> {
    fn into_system(self, ecs: &mut World) -> (SystemFn, SystemDependencies);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_read(id: ResourceId) -> SystemDependencies {
        let mut deps = SystemDependencies::new();
        deps.add_resource_read(id);
        deps
    }

    fn with_write(id: ResourceId) -> SystemDependencies {
        let mut deps = SystemDependencies::new();
        deps.add_resource_write(id);
        deps
    }

    #[test]
    fn shared_reads_are_disjoint() {
        assert!(with_read(1).is_disjoint(&with_read(1)));
    }

    #[test]
    fn read_and_write_conflict() {
        assert!(!with_read(1).is_disjoint(&with_write(1)));
        assert!(!with_write(1).is_disjoint(&with_read(1)));
    }

    #[test]
    fn writes_conflict() {
        assert!(!with_write(1).is_disjoint(&with_write(1)));
    }

    #[test]
    fn component_clash_rejects_unrelated_resources() {
        let mut left = SystemDependencies::new();
        let mut right = SystemDependencies::new();
        left.add_components(ArchetypeKey::EMPTY.with_id(1));
        right.add_components(ArchetypeKey::EMPTY.with_id(1));
        left.add_resource_read(1);
        right.add_resource_read(2);
        assert!(!left.is_disjoint(&right));
    }
}
