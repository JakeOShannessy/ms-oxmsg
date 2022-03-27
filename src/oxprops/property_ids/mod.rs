pub mod lids;
pub mod names;
pub mod tags;
use self::{lids::Lid, names::Name, tags::Tag};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pid {
    Lid(Lid),
    Name(Name),
    Tag(Tag),
}

impl Pid {
    pub fn from_u16(n: u16) -> Self {
        Self::Tag(Tag::from_u16(n))
    }
    pub fn to_u16(self) -> Option<u16> {
        match self {
            Self::Tag(tag) => Some(tag.to_u16()),
            _ => None,
        }
    }
}
