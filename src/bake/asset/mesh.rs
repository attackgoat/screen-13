use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Mesh {
    dst_name: Option<String>,
    src_name: String,
}

impl Mesh {
    pub fn dst_name(&self) -> Option<&str> {
        self.dst_name.as_deref()
    }

    pub fn src_name(&self) -> &str {
        &self.src_name
    }
}
