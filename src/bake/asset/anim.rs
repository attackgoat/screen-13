use {
    serde::Deserialize,
    std::path::{Path, PathBuf},
};

/// Holds a description of `.glb` or `.gltf` model animations.
#[derive(Clone, Deserialize)]
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
