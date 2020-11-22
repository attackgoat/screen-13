use {
    bincode::serialize_into,
    serde::{Deserialize, Serialize},
    std::{
        fmt::{Debug, Formatter, Result},
        ops::Range,
    },
};

/// Exists only to allow data to be stored "raw" instead of letting Bincode do the work
#[derive(Deserialize, PartialEq, Serialize)]
pub enum DataRef<T> {
    Data(T),
    Ref(Range<u32>),
}

impl<T> DataRef<T> {
    pub fn data(&self) -> &T {
        match self {
            Self::Data(ref t) => t,
            _ => panic!("This DataRef is a ref when it should be data"),
        }
    }

    pub fn pos_len(&self) -> (u64, usize) {
        match self {
            Self::Ref(range) => (range.start as _, (range.end - range.start) as _),
            _ => panic!("This DataRef is data when it should be a ref"),
        }
    }

    pub fn is_data(&self) -> bool {
        match self {
            Self::Data(_) => true,
            _ => false,
        }
    }
}

impl<T> DataRef<T>
where
    T: Serialize,
{
    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![];
        serialize_into(&mut buf, self.data()).unwrap();

        buf
    }
}

impl<T> Debug for DataRef<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(match self {
            Self::Data(_) => "Data",
            Self::Ref(_) => "DataRef",
        })
    }
}
