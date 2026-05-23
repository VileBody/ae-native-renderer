use crate::{ArePayloadBacking, ArePayloadBackingId, ArePayloadWindow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorBuilderRoot {
    pub descriptors: Vec<AreEventDescriptor>,
    pub descriptor_vector: AreEventDescriptorVector,
    pub ctx_08: AreClassVector,
    pub ctx_58: AreClassVector,
    pub registrations: Vec<AreDescriptorRegistration>,
    pub payload_allocator: ArePayloadBackingAllocator,
}

impl AreDescriptorBuilderRoot {
    pub fn new() -> Self {
        Self {
            descriptors: Vec::new(),
            descriptor_vector: AreEventDescriptorVector::new(),
            ctx_08: AreClassVector::new(AreClassVectorKind::Ctx08),
            ctx_58: AreClassVector::new(AreClassVectorKind::Ctx58),
            registrations: Vec::new(),
            payload_allocator: ArePayloadBackingAllocator::new(),
        }
    }

    pub fn with_descriptor_capacity(capacity: usize) -> Self {
        let mut root = Self::new();
        root.descriptor_vector =
            AreEventDescriptorVector::with_backing_capacity(&mut root.payload_allocator, capacity);
        root
    }

    pub fn define_descriptor(
        &mut self,
        triplet: AreDescriptorTriplet,
        source_object: AreEventDescriptorSourceObject,
    ) -> DescriptorId {
        let id = DescriptorId(self.descriptors.len());
        self.descriptors
            .push(AreEventDescriptor::new(id, triplet, source_object));
        self.descriptor_vector.push(AreDescriptorRef { id });
        id
    }

    pub fn register_descriptor(&mut self, id: DescriptorId) -> Vec<AreDescriptorRegistration> {
        let descriptor_ref = AreDescriptorRef { id };
        let Some(descriptor) = self.descriptors.get(id.0) else {
            return Vec::new();
        };

        let mut registrations = Vec::new();
        if descriptor.source_object.slot_0x18.is_some() {
            let registration =
                AreDescriptorRegistration::new(descriptor_ref, AreClassVectorKind::Ctx08, 0x18);
            self.ctx_08.push(descriptor_ref);
            self.registrations.push(registration);
            registrations.push(registration);
        }
        if descriptor.source_object.slot_0x20.is_some() {
            let registration =
                AreDescriptorRegistration::new(descriptor_ref, AreClassVectorKind::Ctx58, 0x20);
            self.ctx_58.push(descriptor_ref);
            self.registrations.push(registration);
            registrations.push(registration);
        }
        registrations
    }

    pub fn define_and_register_descriptor(
        &mut self,
        triplet: AreDescriptorTriplet,
        source_object: AreEventDescriptorSourceObject,
    ) -> (DescriptorId, Vec<AreDescriptorRegistration>) {
        let id = self.define_descriptor(triplet, source_object);
        let registrations = self.register_descriptor(id);
        (id, registrations)
    }

    pub fn descriptor(&self, id: DescriptorId) -> Option<&AreEventDescriptor> {
        self.descriptors.get(id.0)
    }

    pub fn descriptor_mut(&mut self, id: DescriptorId) -> Option<&mut AreEventDescriptor> {
        self.descriptors.get_mut(id.0)
    }

    pub fn set_descriptor_payload_window(
        &mut self,
        id: DescriptorId,
        payload_window: ArePayloadWindow,
    ) -> bool {
        let Some(descriptor) = self.descriptor_mut(id) else {
            return false;
        };
        descriptor.payload_window = Some(payload_window);
        true
    }

    pub fn set_descriptor_state(&mut self, id: DescriptorId, state: u8) -> bool {
        let Some(descriptor) = self.descriptor_mut(id) else {
            return false;
        };
        descriptor.state = state;
        true
    }
}

impl Default for AreDescriptorBuilderRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventDescriptor {
    pub id: DescriptorId,
    pub triplet: AreDescriptorTriplet,
    pub cursor_0x18: usize,
    pub cursor_0x20: usize,
    pub source_object: AreEventDescriptorSourceObject,
    pub payload_window: Option<ArePayloadWindow>,
    pub state: u8,
}

impl AreEventDescriptor {
    pub fn new(
        id: DescriptorId,
        triplet: AreDescriptorTriplet,
        source_object: AreEventDescriptorSourceObject,
    ) -> Self {
        Self {
            id,
            cursor_0x18: triplet.base,
            cursor_0x20: triplet.base,
            triplet,
            source_object,
            payload_window: None,
            state: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorTriplet {
    pub base: usize,
    pub second: usize,
    pub source: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventDescriptorSourceObject {
    pub slot_0x18: Option<usize>,
    pub slot_0x20: Option<usize>,
}

impl AreEventDescriptorSourceObject {
    pub fn empty() -> Self {
        Self {
            slot_0x18: None,
            slot_0x20: None,
        }
    }

    pub fn with_0x18(value: usize) -> Self {
        Self {
            slot_0x18: Some(value),
            slot_0x20: None,
        }
    }

    pub fn with_0x20(value: usize) -> Self {
        Self {
            slot_0x18: None,
            slot_0x20: Some(value),
        }
    }

    pub fn with_0x18_0x20(slot_0x18: usize, slot_0x20: usize) -> Self {
        Self {
            slot_0x18: Some(slot_0x18),
            slot_0x20: Some(slot_0x20),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DescriptorId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AreDescriptorRef {
    pub id: DescriptorId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreEventDescriptorVector {
    pub elements: Vec<AreDescriptorRef>,
    pub backing_window: Option<ArePayloadWindow>,
    pub capacity: usize,
}

impl AreEventDescriptorVector {
    pub const ELEMENT_SIZE_BYTES: usize = 8;

    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            backing_window: None,
            capacity: 0,
        }
    }

    pub fn with_backing_capacity(
        allocator: &mut ArePayloadBackingAllocator,
        capacity: usize,
    ) -> Self {
        let backing_window = allocator.allocate_pointer_vector_capacity(capacity);
        Self {
            elements: Vec::with_capacity(capacity),
            backing_window: Some(backing_window),
            capacity,
        }
    }

    pub fn push(&mut self, descriptor_ref: AreDescriptorRef) {
        self.elements.push(descriptor_ref);
    }

    pub fn element_size_bytes(&self) -> usize {
        Self::ELEMENT_SIZE_BYTES
    }
}

impl Default for AreEventDescriptorVector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreClassVector {
    pub kind: AreClassVectorKind,
    pub descriptors: Vec<AreDescriptorRef>,
}

impl AreClassVector {
    pub fn new(kind: AreClassVectorKind) -> Self {
        Self {
            kind,
            descriptors: Vec::new(),
        }
    }

    pub fn push(&mut self, descriptor_ref: AreDescriptorRef) {
        self.descriptors.push(descriptor_ref);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreClassVectorKind {
    Ctx08,
    Ctx58,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreDescriptorRegistration {
    pub descriptor: AreDescriptorRef,
    pub class_vector: AreClassVectorKind,
    pub source_probe_offset: u32,
}

impl AreDescriptorRegistration {
    pub fn new(
        descriptor: AreDescriptorRef,
        class_vector: AreClassVectorKind,
        source_probe_offset: u32,
    ) -> Self {
        Self {
            descriptor,
            class_vector,
            source_probe_offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArePayloadBackingAllocator {
    pub backings: Vec<ArePayloadBacking>,
    pub next_backing_id: usize,
}

impl ArePayloadBackingAllocator {
    pub fn new() -> Self {
        Self {
            backings: Vec::new(),
            next_backing_id: 0,
        }
    }

    pub fn allocate(&mut self, bytes: Vec<u8>) -> ArePayloadWindow {
        let id = ArePayloadBackingId(self.next_backing_id);
        self.next_backing_id += 1;
        let len = bytes.len();
        self.backings.push(ArePayloadBacking { id, bytes });
        ArePayloadWindow {
            backing_id: id,
            offset: 0,
            len,
        }
    }

    pub fn allocate_pointer_vector_capacity(&mut self, capacity: usize) -> ArePayloadWindow {
        self.allocate(vec![
            0;
            capacity * AreEventDescriptorVector::ELEMENT_SIZE_BYTES
        ])
    }

    pub fn backing(&self, id: ArePayloadBackingId) -> Option<&ArePayloadBacking> {
        self.backings.iter().find(|backing| backing.id == id)
    }
}

impl Default for ArePayloadBackingAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod descriptor_builder_tests {
    use super::*;

    fn triplet() -> AreDescriptorTriplet {
        AreDescriptorTriplet {
            base: 0x1000,
            second: 0x2000,
            source: 0x3000,
        }
    }

    #[test]
    fn descriptor_vector_element_is_ref() {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(triplet(), AreEventDescriptorSourceObject::empty());

        assert_eq!(root.descriptor_vector.element_size_bytes(), 8);
        assert_eq!(
            root.descriptor_vector.elements,
            vec![AreDescriptorRef { id }]
        );
    }

    #[test]
    fn descriptor_registration_ctx_08() {
        let mut root = AreDescriptorBuilderRoot::new();
        let (id, registrations) = root.define_and_register_descriptor(
            triplet(),
            AreEventDescriptorSourceObject::with_0x18(0xaaa0),
        );

        assert_eq!(
            registrations,
            vec![AreDescriptorRegistration::new(
                AreDescriptorRef { id },
                AreClassVectorKind::Ctx08,
                0x18,
            )]
        );
        assert_eq!(root.ctx_08.descriptors, vec![AreDescriptorRef { id }]);
        assert!(root.ctx_58.descriptors.is_empty());
    }

    #[test]
    fn descriptor_registration_ctx_58() {
        let mut root = AreDescriptorBuilderRoot::new();
        let (id, registrations) = root.define_and_register_descriptor(
            triplet(),
            AreEventDescriptorSourceObject::with_0x20(0xbbb0),
        );

        assert_eq!(
            registrations,
            vec![AreDescriptorRegistration::new(
                AreDescriptorRef { id },
                AreClassVectorKind::Ctx58,
                0x20,
            )]
        );
        assert!(root.ctx_08.descriptors.is_empty());
        assert_eq!(root.ctx_58.descriptors, vec![AreDescriptorRef { id }]);
    }

    #[test]
    fn source_object_0x18_registers_class_vector() {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x18(1));

        root.register_descriptor(id);

        assert_eq!(root.ctx_08.kind, AreClassVectorKind::Ctx08);
        assert_eq!(root.ctx_08.descriptors, vec![AreDescriptorRef { id }]);
        assert!(root
            .registrations
            .iter()
            .any(|registration| registration.source_probe_offset == 0x18));
    }

    #[test]
    fn source_object_0x20_registers_class_vector() {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(triplet(), AreEventDescriptorSourceObject::with_0x20(1));

        root.register_descriptor(id);

        assert_eq!(root.ctx_58.kind, AreClassVectorKind::Ctx58);
        assert_eq!(root.ctx_58.descriptors, vec![AreDescriptorRef { id }]);
        assert!(root
            .registrations
            .iter()
            .any(|registration| registration.source_probe_offset == 0x20));
    }

    #[test]
    fn no_inline_class_tag_required() {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(
            triplet(),
            AreEventDescriptorSourceObject::with_0x18_0x20(1, 2),
        );

        let descriptor_json = serde_json::to_value(root.descriptor(id).unwrap()).unwrap();
        root.register_descriptor(id);

        assert!(descriptor_json.get("class_tag").is_none());
        assert_eq!(
            root.registrations
                .iter()
                .map(|registration| registration.class_vector)
                .collect::<Vec<_>>(),
            vec![AreClassVectorKind::Ctx08, AreClassVectorKind::Ctx58]
        );
    }

    #[test]
    fn payload_backing_allocates_pointer_vector_capacity() {
        let root = AreDescriptorBuilderRoot::with_descriptor_capacity(3);

        let window = root.descriptor_vector.backing_window.unwrap();
        let backing = root
            .payload_allocator
            .backing(window.backing_id)
            .expect("backing exists");

        assert_eq!(root.descriptor_vector.capacity, 3);
        assert_eq!(window.len, 3 * AreEventDescriptorVector::ELEMENT_SIZE_BYTES);
        assert_eq!(backing.bytes.len(), 24);
    }

    #[test]
    fn descriptor_triplet_resets_cursors_to_base() {
        let mut root = AreDescriptorBuilderRoot::new();
        let id = root.define_descriptor(
            AreDescriptorTriplet {
                base: 0x44,
                second: 0x55,
                source: 0x66,
            },
            AreEventDescriptorSourceObject::empty(),
        );

        let descriptor = root.descriptor(id).unwrap();
        assert_eq!(descriptor.cursor_0x18, 0x44);
        assert_eq!(descriptor.cursor_0x20, 0x44);
    }
}
