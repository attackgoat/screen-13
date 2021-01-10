use {
    super::{MaterialId, ModelId},
    crate::math::{Quat, Vec3},
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

type Idx = u16;

#[derive(Default)]
pub struct Instance {
    pub id: Option<String>,
    pub material: Option<MaterialId>,
    pub model: Option<ModelId>,
    pub position: Vec3,
    pub rotation: Quat,
    pub tags: Vec<String>,
}

/// An individual `Scene` reference.
#[derive(Debug)]
pub struct Ref<'a> {
    idx: usize,
    scene: &'a Scene,
}

impl Ref<'_> {
    /// Returns `true` if the scene contains the given tag.
    pub fn has_tag<T: AsRef<str>>(&self, tag: T) -> bool {
        let tag = tag.as_ref();
        self.scene_ref()
            .tags
            .binary_search_by(|probe| self.scene_str(*probe).cmp(tag))
            .is_ok()
    }

    /// Returns `id`, if set.
    pub fn id(&self) -> Option<&str> {
        self.scene.refs[self.idx]
            .id
            .map(|idx| self.scene.strs[idx as usize].as_str())
    }

    /// Returns `material`, if set.
    pub fn material(&self) -> Option<MaterialId> {
        self.scene_ref().material
    }

    /// Returns `model`, if set.
    pub fn model(&self) -> Option<ModelId> {
        self.scene_ref().model
    }

    /// Returns `position` or the zero vector.
    pub fn position(&self) -> Vec3 {
        self.scene_ref().position
    }

    /// Returns `rotation` or the identity quaternion.
    pub fn rotation(&self) -> Quat {
        self.scene_ref().rotation
    }

    fn scene_ref(&self) -> &SceneRef {
        &self.scene.refs[self.idx]
    }

    fn scene_str<I: Into<usize>>(&self, idx: I) -> &str {
        self.scene.strs[idx.into()].as_str()
    }

    /// Returns an `Iterator` of tags.
    pub fn tags(&self) -> impl Iterator<Item = &str> {
        self.scene_ref()
            .tags
            .iter()
            .map(move |idx| self.scene_str(*idx))
    }
}

/// An `Iterator` of [`Ref`] items.
#[derive(Debug)]
pub struct RefIter<'a> {
    idx: usize,
    scene: &'a Scene,
}

impl<'a> Iterator for RefIter<'a> {
    type Item = Ref<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.scene.refs.len() {
            let res = Ref {
                scene: self.scene,
                idx: self.idx,
            };
            self.idx += 1;
            Some(res)
        } else {
            None
        }
    }
}

/// A container for [`Ref`] references.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Scene {
    refs: Vec<SceneRef>,
    strs: Vec<String>,
}

impl Scene {
    pub(crate) fn new<I: Iterator<Item = Instance>>(instances: I) -> Self {
        let mut refs = vec![];
        let mut strs = vec![];

        // Use a cached index function (idx) here
        let mut cache = HashMap::new();
        let mut idx = |s: String| -> Idx {
            *cache.entry(s.clone()).or_insert_with(|| {
                let res = strs.len() as Idx;
                strs.push(s);
                res
            })
        };

        for mut instance in instances {
            refs.push(SceneRef {
                id: instance.id.map(&mut idx),
                model: instance.model,
                material: instance.material,
                position: instance.position,
                rotation: instance.rotation,
                tags: instance.tags.drain(..).map(&mut idx).collect(),
            });
        }

        Self { refs, strs }
    }

    /// Gets an iterator of the `Ref` instances stored in this `Scene`.
    pub fn refs(&self) -> RefIter {
        RefIter {
            idx: 0,
            scene: self,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct SceneRef {
    id: Option<Idx>,
    model: Option<ModelId>,
    material: Option<MaterialId>,
    position: Vec3,
    rotation: Quat,
    tags: Vec<Idx>,
}
