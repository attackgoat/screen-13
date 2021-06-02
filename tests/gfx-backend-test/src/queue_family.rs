use super::*;

#[derive(Debug)]
pub struct QueueFamilyMock;

impl QueueFamily for QueueFamilyMock {
    fn queue_type(&self) -> QueueType {
        QueueType::General
    }

    fn max_queues(&self) -> usize {
        1
    }

    fn id(&self) -> QueueFamilyId {
        QUEUE_FAMILY_ID
    }

    fn supports_sparse_binding(&self) -> bool {
        false
    }
}
