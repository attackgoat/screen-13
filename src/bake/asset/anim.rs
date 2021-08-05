use {
    super::{Asset, Canonicalize},
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of `.glb` or `.gltf` model animations.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
pub struct Animation {
    exclude: Option<Vec<String>>,
    name: Option<String>,
    src: PathBuf,
}

impl Animation {
    // pub(crate) fn new<E: Iterator<Item = String>, P: AsRef<Path>>(
    //     src: P,
    //     name: Option<String>,
    //     exclude: E,
    // ) -> Self {
    //     let exclude = exclude.collect::<Vec<_>>();
    //     let exclude = if exclude.is_empty() {
    //         None
    //     } else {
    //         Some(exclude)
    //     };

    //     Self {
    //         exclude,
    //         name,
    //         src: src.as_ref().to_owned(),
    //     }
    // }

    /// The bones which were excluded when reading the animation file.
    pub fn exclude(&self) -> Option<&[String]> {
        self.exclude.as_deref()
    }

    /// The name of the animation within the animation file.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The animation file source.
    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}

impl Canonicalize for Animation {
    fn canonicalize<P1, P2>(&mut self, project_dir: P1, src_dir: P2)
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        self.src = Self::canonicalize_project_path(project_dir, src_dir, &self.src);
    }
}

impl From<Animation> for Asset {
    fn from(anim: Animation) -> Self {
        Self::Animation(anim)
    }
}
