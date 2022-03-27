use std::io::{Read, Seek};

use serde::{Deserialize, Serialize};

use crate::read;

#[derive(Clone, Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct AttachmentData {
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Attachment {
    pub cfb_name: String,
    pub name: String,
    pub data: Option<AttachmentData>,
}

impl Attachment {
    /// TODO: switch to stream reading.
    pub fn from_cfb<F: Seek + Read>(
        comp: &mut cfb::CompoundFile<F>,
        cfb_name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let properties_path = format!("/{cfb_name}\\");
        let attachment_properties = crate::parse_property_stream_other(comp, &properties_path);
        let name = {
            let name_path = format!("/{cfb_name}\\__substg1.0_3001001F");
            let mut name_stream = comp.open_stream(&name_path)?;
            let buffer = {
                let mut buffer = Vec::new();
                name_stream.read_to_end(&mut buffer)?;
                buffer
            };
            read(&buffer)?
        };
        let data = {
            let name_path = format!("{cfb_name}\\__substg1.0_37010102");
            if let Ok(mut name_stream) = comp.open_stream(&name_path) {
                let bytes = {
                    let mut buffer = Vec::new();
                    name_stream.read_to_end(&mut buffer)?;
                    buffer
                };
                Some(AttachmentData { bytes })
            } else {
                None
            }
        };

        Ok(Self {
            cfb_name: cfb_name.to_string(),
            name,
            data,
        })
    }
}
