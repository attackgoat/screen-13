use {
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Animation {
    exclude: Option<Vec<String>>,
    name: Option<String>,
    src: PathBuf,
}

impl Animation {
    pub fn new<E: Iterator<Item = String>, P: AsRef<Path>>(
        src: P,
        name: Option<String>,
        exclude: E,
    ) -> Self {
        let exclude = exclude.collect::<Vec<_>>();
        let exclude = if exclude.is_empty() {
            None
        } else {
            Some(exclude)
        };

        Self {
            exclude,
            name,
            src: src.as_ref().to_owned(),
        }
    }

    pub fn exclude(&self) -> Option<&[String]> {
        self.exclude.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn src(&self) -> &Path {
        self.src.as_path()
    }
}
