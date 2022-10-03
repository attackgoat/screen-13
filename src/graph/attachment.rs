use {
    super::{super::driver::image::SampleCount, AttachmentIndex, NodeIndex},
    ash::vk,
    std::collections::BTreeSet,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attachment {
    pub aspect_mask: vk::ImageAspectFlags,
    pub fmt: vk::Format,
    pub sample_count: SampleCount,
    pub target: NodeIndex,
}

impl Attachment {
    fn are_compatible(lhs: Option<Self>, rhs: Option<Self>) -> bool {
        // Two attachment references are compatible if they have matching format and sample
        // count, or are both VK_ATTACHMENT_UNUSED or the pointer that would contain the
        // reference is NULL.
        if lhs.is_none() || rhs.is_none() {
            return true;
        }

        Self::are_identical(lhs.unwrap(), rhs.unwrap())
    }

    pub fn are_identical(lhs: Self, rhs: Self) -> bool {
        lhs.fmt == rhs.fmt && lhs.sample_count == rhs.sample_count
    }
}

#[derive(Clone, Debug, Default)]
pub struct AttachmentMap {
    colors: Vec<Option<Attachment>>,
    depth_stencil: Option<Attachment>,
}

impl AttachmentMap {
    pub fn are_compatible(&self, other: &Self) -> bool {
        // Count of the color attachments may differ, the extras are VK_ATTACHMENT_UNUSED
        self.colors
            .iter()
            .zip(other.colors.iter())
            .all(|(lhs, rhs)| Attachment::are_compatible(*lhs, *rhs))
    }

    pub fn color(&self, attachment_idx: AttachmentIndex) -> Option<Attachment> {
        self.colors.get(attachment_idx as usize).copied().flatten()
    }

    pub fn colors(&self) -> impl Iterator<Item = (AttachmentIndex, Attachment)> + '_ {
        self.colors
            .iter()
            .enumerate()
            .filter_map(|(attachment_idx, opt)| {
                opt.map(|attachment| (attachment_idx as AttachmentIndex, attachment))
            })
    }

    pub fn contains_color(&self, attachment_idx: AttachmentIndex) -> bool {
        self.colors.get(attachment_idx as usize).is_some()
    }

    pub fn contains_image(&self, node_idx: NodeIndex) -> bool {
        // TODO: https://github.com/rust-lang/rust/issues/93050
        if let Some(attachment) = self.depth_stencil {
            if attachment.target == node_idx {
                return true;
            }
        }

        self.colors
            .iter()
            .any(|attachment| matches!(attachment, Some(Attachment { target, .. }) if *target == node_idx))
    }

    pub fn depth_stencil(&self) -> Option<Attachment> {
        self.depth_stencil
    }

    /// Returns true if the previous attachment was compatible
    pub fn insert_color(
        &mut self,
        attachment_idx: AttachmentIndex,
        aspect_mask: vk::ImageAspectFlags,
        fmt: vk::Format,
        sample_count: SampleCount,
        target: NodeIndex,
    ) -> bool {
        // Extend the data as needed
        {
            let attachment_count = attachment_idx as usize + 1;
            if attachment_count > self.colors.len() {
                self.colors.reserve(attachment_count - self.colors.len());
                while self.colors.len() < attachment_count {
                    self.colors.push(None);
                }
            }
        }

        Self::set_attachment(
            &mut self.colors[attachment_idx as usize],
            Attachment {
                aspect_mask,
                fmt,
                sample_count,
                target,
            },
        )
    }

    // Returns the unique targets of this instance.
    pub fn images(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        let mut already_seen = BTreeSet::new();
        self.colors
            .iter()
            .filter_map(|attachment| attachment.as_ref().map(|attachment| attachment.target))
            .chain(
                self.depth_stencil
                    .iter()
                    .map(|attachment| attachment.target),
            )
            .filter(move |&target| already_seen.insert(target))
    }

    /// Returns true if the previous attachment was compatible
    fn set_attachment(curr: &mut Option<Attachment>, next: Attachment) -> bool {
        curr.replace(next)
            .map(|curr| Attachment::are_identical(curr, next))
            .unwrap_or(true)
    }

    /// Returns true if the previous attachment was compatible
    pub fn set_depth_stencil(
        &mut self,
        aspect_mask: vk::ImageAspectFlags,
        fmt: vk::Format,
        sample_count: SampleCount,
        target: NodeIndex,
    ) -> bool {
        Self::set_attachment(
            &mut self.depth_stencil,
            Attachment {
                aspect_mask,
                fmt,
                sample_count,
                target,
            },
        )
    }
}
