use serde::{Deserialize, Serialize};

/// Exists only to allow data to be stored "raw" instead of letting Bincode do the work
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum DataRef<T> {
    Data(T),
    Ref((u32, u32)),
}

impl<T> DataRef<T> {
    pub fn as_data(&self) -> &T {
        match self {
            Self::Data(ref t) => t,
            _ => panic!(
                #[cfg(debug_assertions)]
                "This DataRef is a ref when it should be data"
            ),
        }
    }

    pub fn as_ref(&self) -> (u64, usize) {
        match self {
            Self::Ref((pos, len)) => (*pos as _, *len as _),
            _ => panic!(
                #[cfg(debug_assertions)]
                "This DataRef is data when it should be a ref"
            ),
        }
    }

    pub fn is_data(&self) -> bool {
        match self {
            Self::Data(_) => true,
            _ => false,
        }
    }
}
