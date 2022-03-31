use std::io::{Read, Seek};

use serde::{Deserialize, Serialize};

use crate::{read, PValue};

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Recipient {
    pub cfb_name: String,
    pub address: String,
    // data: Vec<u8>,
}

impl Recipient {
    pub fn from_cfb<F: Seek + Read>(
        comp: &mut cfb::CompoundFile<F>,
        cfb_name: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO work out how to deal with properties.
        let properties_path = format!("/{cfb_name}\\");
        // let properties = crate::parse_property_stream_other(comp, &properties_path);
        // println!("RecipProps {properties:#?}");
        // for property in properties.properties.iter() {
        //     println!("RecipProperty: {property:?}");
        // }
        // "/__recip_version1.0_#00000000\\__properties_version1.0"
        let address = {
            let name_path = format!("{}\\__substg1.0_39FE001F", cfb_name);
            if let Ok(mut name_stream) = comp.open_stream(&name_path) {
                let buffer = {
                    let mut buffer = Vec::new();
                    name_stream.read_to_end(&mut buffer)?;
                    buffer
                };
                Some(read(&buffer)?)
            } else {
                None
            }
        };
        let address = if let Some(address) = address {
            address
        } else {
            let name_path = format!("{}\\__substg1.0_3003001F", cfb_name);
            let mut name_stream = comp.open_stream(&name_path)?;
            let buffer = {
                let mut buffer = Vec::new();
                name_stream.read_to_end(&mut buffer)?;
                buffer
            };
            read(&buffer)?
        };
        // println!("RecipAddress: {address}");
        // let data = {
        //     let name_path = format!("{}\\__substg1.0_37010102", cfb_name);
        //     let mut name_stream = comp.open_stream(&name_path)?;
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         name_stream.read_to_end(&mut buffer)?;
        //         buffer
        //     };
        //     buffer
        // };
        Ok(Self {
            cfb_name,
            address,
            // data,
        })
    }
}
