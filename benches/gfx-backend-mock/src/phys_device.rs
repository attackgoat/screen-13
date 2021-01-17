use {
    super::{Backend, *},
    gfx_hal::{device::CreationError, image::Usage, memory::Properties as MemProperties},
};

#[derive(Debug)]
pub struct PhysicalDeviceMock;

impl PhysicalDevice<Backend> for PhysicalDeviceMock {
    unsafe fn open(
        &self,
        families: &[(&QueueFamilyMock, &[QueuePriority])],
        _requested_features: Features,
    ) -> Result<Gpu<Backend>, CreationError> {
        // Validate the arguments
        assert_eq!(
            families.len(),
            1,
            "Empty backend doesn't have multiple queue families"
        );
        let (_family, priorities) = families[0];
        assert_eq!(
            priorities.len(),
            1,
            "Empty backend doesn't support multiple queues"
        );
        let priority = priorities[0];
        assert!(
            0.0 <= priority && priority <= 1.0,
            "Queue priority is out of range"
        );

        // Create the queues
        let queue_groups = {
            let mut queue_group = QueueGroup::new(QUEUE_FAMILY_ID);
            queue_group.add_queue(CommandQueueMock);
            vec![queue_group]
        };
        let gpu = Gpu {
            device: DeviceMock,
            queue_groups,
        };
        Ok(gpu)
    }

    fn format_properties(&self, _: Option<Format>) -> Properties {
        Properties {
            linear_tiling: ImageFeature::all(),
            optimal_tiling: ImageFeature::all(),
            buffer_features: BufferFeature::all(),
        }
    }

    fn image_format_properties(
        &self,
        _: Format,
        _dim: u8,
        _: Tiling,
        _: Usage,
        _: ViewCapabilities,
    ) -> Option<FormatProperties> {
        todo!()
    }

    fn memory_properties(&self) -> MemoryProperties {
        let memory_types = {
            let properties = MemProperties::DEVICE_LOCAL
                | MemProperties::CPU_VISIBLE
                | MemProperties::COHERENT
                | MemProperties::CPU_CACHED;
            let memory_type = MemoryType {
                properties,
                heap_index: 0,
            };
            vec![memory_type]
        };
        // TODO: perhaps get an estimate of free RAM to report here?
        let memory_heaps = vec![MemoryHeap {
            size: 64 * 1024,
            flags: HeapFlags::empty(),
        }];
        MemoryProperties {
            memory_types,
            memory_heaps,
        }
    }

    fn features(&self) -> Features {
        Features::empty()
    }

    fn capabilities(&self) -> Capabilities {
        Default::default()
    }

    fn limits(&self) -> Limits {
        Limits {
            non_coherent_atom_size: 1,
            optimal_buffer_copy_pitch_alignment: 1,
            ..Default::default()
        }
    }
}
