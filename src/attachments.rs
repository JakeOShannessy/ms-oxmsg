use std::io::{Read, Seek};

use cfb::Entry;
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
            let name_path = format!("/{cfb_name}\\__substg1.0_3707001F");
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

        let storage_path = format!("/{cfb_name}");
        #[allow(clippy::needless_collect)]
        let streams: Vec<Entry> = comp.read_storage(&storage_path).unwrap().collect();
        for (i, s) in streams.into_iter().enumerate() {
            println!("{}", s.path().display());
            // assert!(!s.is_storage());

            if  s.name() == "__properties_version1.0" {
                // These streams have already been read.
                println!("  Stream already parsed");
            } else if s.is_stream() {
                // Read in all the data from one of the streams in that compound file.
                let data = {
                    let mut stream = if let Ok(s) = comp.open_stream(s.path()) {
                        s
                    } else {
                        continue;
                    };
                    let mut buffer = Vec::new();
                    stream.read_to_end(&mut buffer).unwrap();
                    buffer
                };
                print!("  Stream[{}]({})[{}]", i, data.len(), s.path().display());
                if let Ok(recip0) = read(&data) {
                    print!(": {}", recip0);
                }
                println!();
            }
        }

        Ok(Self {
            cfb_name: cfb_name.to_string(),
            name,
            data,
        })
    }
}
