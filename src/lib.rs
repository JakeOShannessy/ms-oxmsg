#![allow(dead_code)]
use chrono::{DateTime, Utc};
use oxprops::property_ids::{tags::Tag, Pid};
use serde::{Deserialize, Serialize};
use std::{
    convert::TryInto,
    io::{Read, Seek, Write},
    path::Path,
};
use uuid::Uuid;

use crate::oxprops::property_sets::PropertySet;
mod oxprops;

/// Iterates up to 2048.
struct AttachmentNameIter {
    i: u16,
}

impl AttachmentNameIter {
    fn new() -> Self {
        Self { i: 0 }
    }
}

impl Iterator for AttachmentNameIter {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= 2048 {
            return None;
        }
        let s = format!("/__attach_version1.0_#0000{:04X}", self.i);
        self.i += 1;
        Some(s)
    }
}

/// Iterates up to 2048.
struct RecipientNameIter {
    i: u16,
}

impl RecipientNameIter {
    fn new() -> Self {
        Self { i: 0 }
    }
}

impl Iterator for RecipientNameIter {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= 2048 {
            return None;
        }
        let s = format!("/__recip_version1.0_#0000{:04X}", self.i);
        self.i += 1;
        Some(s)
    }
}

pub struct RawMsg {}

struct Message {
    string_stream: StringStream,
    guid_stream: GuidStream,
    subject: String,
    sender: String,
    delivery_time: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct EmailMessage {
    // string_stream: StringStream,
    // guid_stream: GuidStream,
    /// Direct hash of the bytes
    // pub hash: ObjectHash,
    pub subject: String,
    pub sender: String,
    pub attachments: Vec<Attachment>,
    pub recipients: Vec<Recipient>,
    pub delivery_time: DateTime<Utc>,
}

impl EmailMessage {
    pub fn is_attached(&self, filename: &str) -> bool {
        for attachment in &self.attachments {
            if attachment.name == filename {
                return true;
            }
        }
        false
    }
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        // We will read the whole email into memory for safety. By reading the
        // whole thing into memory, we know that the library can't make any
        // modifications to it.
        let mut file = std::fs::File::open(&path)?;
        // Read that file into a buffer.
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        // let hash = {
        //     let mut hasher = blake2::Blake2b::with_params(&[], &[], &[]);
        //     hasher.update(&buffer);
        //     let hash = hasher.finalize();
        //     ObjectHash::Blake2b(hash.as_slice().try_into().unwrap())
        // };
        let cursor = std::io::Cursor::new(&buffer);
        let mut comp = cfb::CompoundFile::open(cursor)?;

        let mut attachments = Vec::new();

        for name in AttachmentNameIter::new() {
            if comp.exists(&name) {
                match Attachment::from_cfb(&mut comp, name) {
                    Ok(attachment) => attachments.push(attachment),
                    Err(err) => eprintln!("ERR[{}]: {:?}", path.as_ref().display(), err),
                }
            } else {
                break;
            }
        }

        let mut recipients = Vec::new();

        for name in RecipientNameIter::new() {
            if comp.exists(&name) {
                let recipient = Recipient::from_cfb(&mut comp, name)?;
                recipients.push(recipient);
            } else {
                break;
            }
        }
        let subject = {
            let mut stream = comp.open_stream("/__substg1.0_0037001F")?;
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer)?;
                buffer
            };
            read(&buffer)?
        };
        let sender = {
            // let mut stream = comp.open_stream("/__substg1.0_3FFA001F")?;
            let mut stream = comp.open_stream("/__substg1.0_0C1F001F")?;
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer)?;
                buffer
            };
            read(&buffer)?
        };
        let properties = {
            let mut stream = comp.open_stream("/__properties_version1.0")?;
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer)?;
                buffer
            };
            parse_property_stream_top_level(&buffer)
        };
        let mut delivery_time = None;
        for property in properties {
            if property.property_id == Pid::Tag(Tag::MessageDeliveryTime) {
                if let PValue::Time(time) = property.value {
                    delivery_time = Some(time);
                }
            }
        }
        let delivery_time = delivery_time.unwrap();
        Ok(Self {
            // hash,
            subject,
            sender,
            attachments,
            recipients,
            delivery_time,
        })
    }
}

struct StringStream {
    buffer: Vec<u8>,
}

impl StringStream {
    fn new(buffer: Vec<u8>) -> Self {
        Self { buffer }
    }

    fn get_bytes(&self, index: usize) -> &[u8] {
        let length = u32::from_le_bytes([
            self.buffer[index],
            self.buffer[index + 1],
            self.buffer[index + 2],
            self.buffer[index + 3],
        ]) as usize;
        &self.buffer[index + 4..index + length + 4]
    }

    fn get(&self, index: usize) -> Result<String, &'static str> {
        let bytes = self.get_bytes(index);
        read(bytes)
    }
}

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
    fn from_cfb<F: Seek + Read>(
        comp: &mut cfb::CompoundFile<F>,
        cfb_name: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let name = {
            let name_path = format!("{}\\__substg1.0_3001001F", cfb_name);
            let mut name_stream = comp.open_stream(&name_path)?;
            let buffer = {
                let mut buffer = Vec::new();
                name_stream.read_to_end(&mut buffer)?;
                buffer
            };
            read(&buffer)?
        };
        let data = {
            let name_path = format!("{}\\__substg1.0_37010102", cfb_name);
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
            cfb_name,
            name,
            data,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Recipient {
    pub cfb_name: String,
    pub address: String,
    // data: Vec<u8>,
}

impl Recipient {
    fn from_cfb<F: Seek + Read>(
        comp: &mut cfb::CompoundFile<F>,
        cfb_name: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO work out how to deal with properties.

        // "/__recip_version1.0_#00000000\\__properties_version1.0"
        let address = {
            let name_path = format!("{}\\__substg1.0_3003001F", cfb_name);
            let mut name_stream = comp.open_stream(&name_path)?;
            let buffer = {
                let mut buffer = Vec::new();
                name_stream.read_to_end(&mut buffer)?;
                buffer
            };
            read(&buffer)?
        };
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

struct GuidStream {
    buffer: Vec<u8>,
}

impl GuidStream {
    fn new(buffer: Vec<u8>) -> Self {
        Self { buffer }
    }

    fn get_bytes(&self, index: usize) -> &[u8] {
        &self.buffer[index * 16..index * 16 + 16]
    }

    fn get(&self, index: usize) -> Uuid {
        let bytes = self.get_bytes(index);
        parse_guid(bytes)
    }
}

fn parse_guid(data_slice: &[u8]) -> Uuid {
    Uuid::from_u128(u128::from_be_bytes([
        data_slice[3],
        data_slice[2],
        data_slice[1],
        data_slice[0],
        data_slice[5],
        data_slice[4],
        data_slice[7],
        data_slice[6],
        data_slice[8],
        data_slice[9],
        data_slice[10],
        data_slice[11],
        data_slice[12],
        data_slice[13],
        data_slice[14],
        data_slice[15],
    ]))
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct Guid(u128);

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
enum GuidIndex {
    PsMapi,
    PublicStrings,
    StreamIndex(u16),
}

impl GuidIndex {
    fn new(value: u16) -> Self {
        if value < 1 {
            panic!("GUID Index value must be non-zero");
        }
        if value == 1 {
            Self::PsMapi
        } else if value == 2 {
            Self::PublicStrings
        } else {
            Self::StreamIndex(value - 3)
        }
    }

    fn as_num(self) -> u16 {
        match self {
            GuidIndex::PsMapi => 1,
            GuidIndex::PublicStrings => 2,
            GuidIndex::StreamIndex(n) => n + 3,
        }
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
enum PropertyKind {
    Numerical,
    String,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
enum PropertyId {
    Number(u32),
    String(String),
}

impl std::fmt::Debug for PropertyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_tuple("PropertyId");
        match self {
            PropertyId::Number(n) => f.field(&(n)),
            PropertyId::String(s) => f.field(s),
        };
        f.finish()
    }
}
fn read(bytes: &[u8]) -> Result<String, &'static str> {
    let points = read_le_u16(bytes)?;
    let title = String::from_utf16_lossy(&points);
    Ok(title)
}

fn read_le_u16(input: &[u8]) -> Result<Vec<u16>, &'static str> {
    let length = if (input.len() % 2) != 0 {
        return Err("Length must be a multiple of 2");
    } else {
        input.len() / 2
    };
    let mut buffer = Vec::with_capacity(length);
    let mut input = input;
    loop {
        if input.is_empty() {
            break;
        }
        let (int_bytes, rest) = input.split_at(std::mem::size_of::<u16>());
        input = rest;
        buffer.push(u16::from_le_bytes(int_bytes.try_into().unwrap()));
    }
    Ok(buffer)
}

#[derive(Clone, Debug)]
pub struct PropertyMapping {
    property_set: PropertySet,
    // property_index: u16,
    // property_kind: PropertyKind,
    property_name: PropertyMappingIdentifier,
    property_id: u16,
    stream_name: String,
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum PropertyMappingIdentifier {
    Number(u32),
    String([u8; 4]),
}

#[cfg(test)]
mod tests {
    use crate::oxprops::{
        property_ids::{self, lids::Lid},
        property_sets::PropertySet,
    };

    use super::*;
    use cfb::Entry;

    #[test]
    fn simple_email() {
        let email = EmailMessage::from_file("test_email_with_attachments.msg");
        println!("{:#?}", email);
    }

    #[test]
    fn read_to_cfb() {
        use std::io::{Read, Write};
        // We will read the whole email into memory for safety. By reading the
        // whole thing into memory, we know that the library can't make any
        // modifications to it.
        let mut file = std::fs::File::open("test_email.msg").unwrap();
        // Read that file into a buffer.
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let cursor = std::io::Cursor::new(&buffer);
        let mut comp = cfb::CompoundFile::open(cursor).unwrap();
        for e in comp.walk() {
            println!("entry[{}]: {:?}", e.is_storage(), e.path());
        }

        // let string_stream = {
        //     let mut stream = comp
        //         .open_stream("/__nameid_version1.0\\__substg1.0_00040102")
        //         .unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     StringStream { buffer }
        // };
        // let guid_stream = {
        //     let mut stream = comp
        //         .open_stream("/__nameid_version1.0\\__substg1.0_00020102")
        //         .unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     GuidStream { buffer }
        // };
        // let subject = {
        //     let mut stream = comp.open_stream("/__substg1.0_0037001F").unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     read(&buffer).unwrap()
        // };
        // let sender = {
        //     // let mut stream = comp.open_stream("/__substg1.0_3FFA001F")?;
        //     let mut stream = comp.open_stream("/__substg1.0_0C1F001F").unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     read(&buffer).unwrap()
        // };
        // let body = {
        //     // let mut stream = comp.open_stream("/__substg1.0_3FFA001F")?;
        //     let mut stream = comp.open_stream("/__substg1.0_1000001F").unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     read(&buffer).ok()
        // };
        // let properties = {
        //     let mut stream = comp.open_stream("/__properties_version1.0").unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     parse_property_stream_top_level(&buffer)
        // };
        // let mut delivery_time = None;
        // for property in properties {
        //     if property.property_id == Pid::Tag(Tag::MessageDeliveryTime) {
        //         if let PValue::Time(time) = property.value {
        //             delivery_time = Some(time);
        //         }
        //     }
        //     if let PValue::Time(time) = property.value {
        //         println!("{:?}: {time}", property.property_id);
        //     }
        // }
        // let delivery_time = delivery_time.unwrap();
        // let message = Message {
        //     string_stream,
        //     guid_stream,
        //     subject,
        //     sender,
        //     delivery_time,
        // };
        // println!("Subject: {}", message.subject);
        // println!("Sender: {}", message.sender);
        // if let Some(body) = body {
        //     println!("Body: {}", body);
        // }
        // println!("Delivery Time: {}", message.delivery_time);
        let mut string_stream = None;
        let mut guid_stream = None;
        let mut raw_entry_stream = Vec::new();

        let mut entries = Vec::new();
        let mut property_mappings = Vec::new();
        // let string_stream = {
        //     let mut stream = comp
        //         .open_stream("/__nameid_version1.0\\__substg1.0_00040102")
        //         .unwrap();
        //     let buffer = {
        //         let mut buffer = Vec::new();
        //         stream.read_to_end(&mut buffer).unwrap();
        //         buffer
        //     };
        //     StringStream { buffer }
        // };

        #[allow(clippy::needless_collect)]
        let streams: Vec<Entry> = comp.walk().collect();

        if let Ok(mut stream) = comp.open_stream("/__nameid_version1.0\\__substg1.0_00020102") {
            let data = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            let len = data.len();
            println!("GuidStream, len = {len}");
            guid_stream = Some(GuidStream {
                buffer: data.clone(),
            });
            let mut data_slice = data.as_slice();
            let mut n = 0;
            loop {
                if data_slice.is_empty() {
                    break;
                }
                let guid: Uuid = parse_guid(data_slice);
                println!("    GUID[{}]: {}", n, guid);
                data_slice = &data_slice[16..];
                n += 1;
            }
        }

        if let Ok(mut stream) = comp.open_stream("/__nameid_version1.0\\__substg1.0_00040102") {
            let data = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            string_stream = Some(StringStream {
                buffer: data.clone(),
            });
            let len = data.len();
            println!("StringStream, len = {len}");
            let mut data_slice = data.as_slice();
            let mut n = 0;
            loop {
                if data_slice.is_empty() {
                    break;
                }
                let length = u32::from_le_bytes([
                    data_slice[0],
                    data_slice[1],
                    data_slice[2],
                    data_slice[3],
                ]) as usize;
                if length > data_slice.len() {
                    // data_slice = &data_slice[1..];
                    // continue;
                    break;
                }
                data_slice = &data_slice[4..];
                print!("    StringEntry[{}]({})", n, length);
                std::io::stdout().flush().unwrap();
                if let Ok(recip0) = read(&data_slice[0..length]) {
                    print!(": {}", recip0);
                }
                println!();
                let next_offset = length + length % 4;
                // println!("next_offset: {}", next_offset);
                if next_offset > data_slice.len() {
                    break;
                }
                data_slice = &data_slice[next_offset..];
                n += 1;
            }
        }

        if let Ok(mut stream) = comp.open_stream("/__nameid_version1.0\\__substg1.0_00030102") {
            let data = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            raw_entry_stream = data.clone();
            let len = data.len();
            println!("EntryStream, len = {len}");
            entries = parse_properties(
                string_stream.as_ref().unwrap(),
                guid_stream.as_ref().unwrap(),
                data.as_slice(),
            );
        }

        for (i, s) in streams.into_iter().enumerate() {
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
            println!("{}", s.path().display());
            if s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00020102"
                || s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00030102"
                || s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00040102"
            {
                // These streams have already been read.
                println!("  Stream already parsed");
            } else if s.path().starts_with("/__nameid_version1.0") {
                // These are the property mappings (exlcuding the 3 streams already deal with)
                let len = data.len();
                let name = s.name();
                // println!("named property mapping (len = {len}): {name} - {identifier:?} - {index_kind:?}");
                println!("  NamedPropertyMapping (len = {len}): {name}");
                let mut data_slice = data.as_slice();
                let mut n = 0;
                loop {
                    if data_slice.is_empty() {
                        break;
                    }
                    let (property_index, guid_index, property_kind) = parse_kind_index([
                        data_slice[4],
                        data_slice[5],
                        data_slice[6],
                        data_slice[7],
                    ]);
                    let property_set = match guid_index {
                        GuidIndex::PsMapi => PropertySet::PsMapi,
                        GuidIndex::PublicStrings => PropertySet::PublicStrings,
                        GuidIndex::StreamIndex(index) => {
                            let guid = guid_stream.as_ref().unwrap().get(index as usize);
                            PropertySet::from_uuid(guid)
                        }
                    };
                    let identifier = match property_kind {
                        PropertyKind::Numerical => {
                            PropertyMappingIdentifier::Number(u32::from_le_bytes([
                                data_slice[0],
                                data_slice[1],
                                data_slice[2],
                                data_slice[3],
                            ]))
                        }
                        PropertyKind::String => PropertyMappingIdentifier::String([
                            data_slice[0],
                            data_slice[1],
                            data_slice[2],
                            data_slice[3],
                        ]),
                    };
                    let id_string = match property_kind {
                        PropertyKind::Numerical => {
                            let num = u32::from_le_bytes([
                                data_slice[0],
                                data_slice[1],
                                data_slice[2],
                                data_slice[3],
                            ]);
                            format!("id: 0x{num:08X}")
                        }
                        PropertyKind::String => {
                            let num = u32::from_le_bytes([
                                data_slice[0],
                                data_slice[1],
                                data_slice[2],
                                data_slice[3],
                            ]);
                            format!("crc: 0x{num:08X}")
                        }
                    };
                    // println!(
                    //     "    [{n}]: {id_string} property_index: {:?}",
                    //     property_index,
                    // );
                    let stream_id = match identifier {
                        PropertyMappingIdentifier::Number(n) => {
                            0x1000 + ((n as u16) ^ (guid_index.as_num() << 1)) % 0x1F
                        }
                        PropertyMappingIdentifier::String(crc) => {
                            let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
                            let mut digest = crc.digest();
                            digest.update(&data_slice[0..=3]);
                            let checksum = digest.finalize();
                            0x1000 + ((checksum as u16) ^ (guid_index.as_num() << 1 | 1)) % 0x1F
                        }
                    };
                    let hex_id: u32 = ((stream_id as u32) << 16) | 0x00000102;
                    let stream_name = format!("__substg1.0_{:X}", hex_id);
                    let property_id = 0x8000 + property_index;
                    let property_name = identifier;
                    // assert_eq!(stream_name,format!("{}",s.path().display())[21..]);
                    let property_mapping = PropertyMapping {
                        property_set,
                        // property_index,
                        // property_kind,
                        property_name,
                        property_id,
                        stream_name,
                    };
                    let name_string = match property_name {
                        PropertyMappingIdentifier::Number(n) => {
                            format!("id:  0x{n:08X}")
                        }
                        PropertyMappingIdentifier::String(crc) => {
                            let n = u32::from_le_bytes(crc);
                            format!("crc: 0x{n:08X}")
                        }
                    };
                    let entry_offset = ((property_id - 0x8000) as usize) * 8;
                    println!("    {name_string} -> 0x{property_id:02X} {property_mapping:?} entry_offset: {entry_offset}");
                    if let PropertyMappingIdentifier::Number(n) = property_name {
                        if let Some(lid) = Lid::from_u32(n) {
                            println!("    LID: {lid:?}");
                        }
                    }
                    if let Some(entry_data) = raw_entry_stream.get(entry_offset..(entry_offset + 8))
                    {
                        parse_entry(
                            string_stream.as_ref().unwrap(),
                            guid_stream.as_ref().unwrap(),
                            [
                                entry_data[0],
                                entry_data[1],
                                entry_data[2],
                                entry_data[3],
                                entry_data[4],
                                entry_data[5],
                                entry_data[6],
                                entry_data[7],
                            ],
                        );
                    }

                    property_mappings.push(property_mapping);
                    data_slice = &data_slice[8..];
                    n += 1;
                }
            } else if s.path().as_os_str() == "/__attach_version1.0_#00000000\\__substg1.0_3001001F"
            {
                print!("  Stream[{}]({})[{}]", i, data.len(), s.path().display());
                if let Ok(recip0) = read(&data) {
                    print!(": ATTACHMENT: {}", recip0);
                }
                println!();
            } else if s.path().as_os_str() == "/__properties_version1.0" {
                println!("  other properties");
                let properties = parse_property_stream_top_level(&data);
                for property in properties {
                    println!("    0x{:04X} {property:?}",property.property_id.to_u16().unwrap());
                }
            } else {
                print!("  Stream[{}]({})[{}]", i, data.len(), s.path().display());
                if let Ok(recip0) = read(&data) {
                    print!(": {}", recip0);
                }
                println!();
            }
            // s.
        }
    }

    #[ignore]
    #[test]
    fn attach_name_iter() {
        let iter = AttachmentNameIter::new();
        for s in iter {
            println!("{}", s);
        }
    }
}

pub struct Property {
    kind: PropertyKind,
    id: PropertyId,
}
type PropertyIndex = u16;
fn parse_kind_index(data: [u8; 4]) -> (PropertyIndex, GuidIndex, PropertyKind) {
    let property_kind: PropertyKind = if data[0] & 0x1 == 1 {
        PropertyKind::String
    } else {
        PropertyKind::Numerical
    };
    let property_index = u16::from_le_bytes([data[2], data[3]]);
    let guid_index = GuidIndex::new(u16::from_le_bytes([data[0], data[1]]) >> 1);
    (property_index, guid_index, property_kind)
}

#[derive(Clone, Debug)]
pub struct PropertyEntry {
    property_set: PropertySet,
    property_index: u16,
    property_kind: PropertyKind,
    stream_name: String,
}

fn parse_properties(
    string_stream: &StringStream,
    guid_stream: &GuidStream,
    data_slice: &[u8],
) -> Vec<PropertyEntry> {
    let mut data_slice = data_slice;
    let mut n = 0;
    let mut properties = Vec::new();
    loop {
        print!("    [{}]", n * 8);
        let property = parse_entry(
            string_stream,
            guid_stream,
            [
                data_slice[0],
                data_slice[1],
                data_slice[2],
                data_slice[3],
                data_slice[4],
                data_slice[5],
                data_slice[6],
                data_slice[7],
            ],
        );
        properties.push(property);
        data_slice = &data_slice[8..];
        if data_slice.is_empty() {
            break;
        }
        n += 1;
    }
    properties
}

fn parse_entry(
    string_stream: &StringStream,
    guid_stream: &GuidStream,
    data_slice: [u8; 8],
) -> PropertyEntry {
    let (property_index, guid_index, property_kind) =
        parse_kind_index([data_slice[4], data_slice[5], data_slice[6], data_slice[7]]);

    let identifier = match property_kind {
        PropertyKind::Numerical => PropertyId::Number(u32::from_le_bytes([
            data_slice[0],
            data_slice[1],
            data_slice[2],
            data_slice[3],
        ])),
        PropertyKind::String => {
            let num =
                u32::from_le_bytes([data_slice[0], data_slice[1], data_slice[2], data_slice[3]]);
            PropertyId::String(string_stream.get(num as usize).unwrap())
        }
    };
    println!("    PropertyEntry[{property_index}]: Id: {identifier:?} PropertyIndex: {property_index} GuidIndex: {guid_index:?}");
    println!(
        "        {:02X} {:02X} {:02X} {:02X}",
        data_slice[0], data_slice[1], data_slice[2], data_slice[3]
    );
    println!(
        "        {:02X} {:02X} {:02X} {:02X}",
        data_slice[4], data_slice[5], data_slice[6], data_slice[7]
    );
    std::io::stdout().flush().unwrap();
    let property_set = match guid_index {
        GuidIndex::PsMapi => PropertySet::PsMapi,
        GuidIndex::PublicStrings => PropertySet::PublicStrings,
        GuidIndex::StreamIndex(index) => {
            let guid = guid_stream.get(index as usize);
            let property_set = PropertySet::from_uuid(guid);
            print!("        PropertySet: {:?}", property_set);
            property_set
        }
    };
    std::io::stdout().flush().unwrap();
    println!();
    let stream_id = match identifier {
        PropertyId::Number(n) => 0x1000 + ((n as u16) ^ (guid_index.as_num() << 1)) % 0x1F,
        PropertyId::String(_s) => {
            let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
            let mut digest = crc.digest();
            digest.update(&data_slice[0..=3]);
            let checksum = digest.finalize();
            0x1000 + ((checksum as u16) ^ (guid_index.as_num() << 1 | 1)) % 0x1F
        }
    };
    let hex_id: u32 = ((stream_id as u32) << 16) | 0x00000102;
    let stream_name = format!("__substg1.0_{:X}", hex_id);
    println!("        stream_name: {}", stream_name);
    PropertyEntry {
        property_set,
        property_index,
        property_kind,
        stream_name,
    }
}

// pub fn id_to_stream_id(guid_index:GuidIndex, identifier:PropertyId) {
//     let stream_id = match identifier {
//         PropertyId::Number(n) => 0x1000 + ((n as u16) ^ (guid_index.as_num() << 1)) % 0x1F,
//         PropertyId::String(_s) => {
//             let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
//             let mut digest = crc.digest();
//             digest.update(&data_slice[0..=3]);
//             let checksum = digest.finalize();
//             0x1000 + ((checksum as u16) ^ (guid_index.as_num() << 1 | 1)) % 0x1F
//         }
//     };
// }
// pub fn stream_id_to_stream_name(stream_id:u16) ->String {
//     let hex_id: u32 = ((stream_id as u32) << 16) | 0x00000102;
//     let stream_name = format!("__substg1.0_{:X}", hex_id);
//     stream_name
// }

// PidTagMessageDeliveryTime: 0x0E06
// PidTagSenderEmailAddress: 0x0C1F
// PidTagClientSubmitTime: 0x0039

fn parse_property_stream_top_level(data_slice: &[u8]) -> Vec<FixedLengthPropertyEntry> {
    let mut data_slice = data_slice;
    // Ignore the first 8 bytes as required by spec.
    let _reserved1 = &data_slice[0..8];
    let next_recipient_id = &data_slice[8..12];
    let next_attachment_id = &data_slice[12..16];
    let recipient_count = &data_slice[16..20];
    let attachment_count = &data_slice[20..24];
    let _reserved2 = &data_slice[24..32];
    data_slice = &data_slice[32..];
    let mut n = 0;
    let mut properties = Vec::new();
    loop {
        let property = parse_fixed_length_property_entry([
            data_slice[0],
            data_slice[1],
            data_slice[2],
            data_slice[3],
            data_slice[4],
            data_slice[5],
            data_slice[6],
            data_slice[7],
            data_slice[8],
            data_slice[9],
            data_slice[10],
            data_slice[11],
            data_slice[12],
            data_slice[13],
            data_slice[14],
            data_slice[15],
        ]);
        properties.push(property);
        data_slice = &data_slice[16..];
        if data_slice.is_empty() {
            break;
        }
        n += 1;
    }
    properties
}

bitflags::bitflags! {
    pub struct Flags: u32 {
        const PROPATTR_MANDATORY = 0x00000001;
        const PROPATTR_READABLE = 0x00000002;
        const PROPATTR_WRITABLE = 0x00000004;
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct FixedLengthPropertyEntry {
    pub property_id: Pid,
    pub flags: Flags,
    pub value: PValue,
}

// TODO: replace many of these values with ms-dtype
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum PValue {
    Integer16(i16),
    Integer32(i32),
    Floating32(f32),
    Floating64(f64),
    Currency(i64),
    FloatingTime(f64),
    ErrorCode,
    Boolean(bool),
    Integer64(i64),
    String(u32),
    String8(u32),
    Time(DateTime<Utc>),
    Guid(Uuid),
    ServerId(u32),    // TODO: check
    Restriction(u32), // TODO: check
    RuleAction(u32),  // TODO: check
    Binary(u32),
    MultipleInteger16(u32),
    MultipleInteger32(u32),
    MultipleFloating32(u32),
    MultipleFloating64(u32),
    MultipleCurrency(u32),
    MultipleFloatingTime(u32),
    MultipleInteger64(u32),
    MultipleString(u32),
    MultipleString8(u32),
    MultipleTime(u32),
    MultipleGuid(u32),
    MultipleBinary(u32),
    Unspecified(u32),
    Null,
    Object,
}

impl PValue {
    pub fn from_bytes(property_type: PType, data: [u8; 8]) -> PValue {
        match property_type {
            PType::Integer16 => PValue::Integer16(i16::from_le_bytes([data[0], data[1]])),
            PType::Integer32 => {
                PValue::Integer32(i32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Floating32 => {
                PValue::Floating32(f32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Floating64 => PValue::Floating64(f64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ])),
            PType::Currency => PValue::Currency(i64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ])),
            PType::FloatingTime => PValue::FloatingTime(f64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ])),
            PType::ErrorCode => PValue::ErrorCode,
            PType::Boolean => PValue::Boolean(u8::from_le(data[0]) == 1_u8),
            PType::Integer64 => PValue::Integer64(i64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ])),
            PType::String => {
                PValue::String(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::String8 => {
                PValue::String8(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Time => {
                // parse time
                let nano_100s = i64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                eprint!(" time: {nano_100s}");
                let origin_seconds = chrono::NaiveDate::from_ymd(1970, 1, 1)
                    .and_hms_milli(0, 0, 0, 0)
                    .timestamp()
                    - chrono::NaiveDate::from_ymd(1601, 1, 1)
                        .and_hms_milli(0, 0, 0, 0)
                        .timestamp();
                let time_seconds = nano_100s / 10 / 1000 / 1000 - origin_seconds;
                let time_nanoseconds = (nano_100s % (10_000_000)).abs() as u32;
                eprint!(" time seconds: {time_seconds} s");
                eprint!(" time nanoseconds: {time_nanoseconds} ns");
                let time = chrono::NaiveDateTime::from_timestamp(time_seconds, time_nanoseconds);
                // Time is UTC as per MS-OXPROPS
                let utc_time: DateTime<Utc> = chrono::DateTime::from_utc(time, chrono::Utc);
                eprint!(" time: {time}");
                PValue::Time(utc_time)
            }
            PType::Guid => panic!("guid"),
            PType::ServerId => {
                PValue::ServerId(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Restriction => {
                PValue::Restriction(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::RuleAction => {
                PValue::RuleAction(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Binary => {
                PValue::Binary(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleInteger16 => {
                PValue::MultipleInteger16(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleInteger32 => {
                PValue::MultipleInteger32(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleFloating32 => {
                PValue::MultipleFloating32(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleFloating64 => {
                PValue::MultipleFloating64(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleCurrency => {
                PValue::MultipleCurrency(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleFloatingTime => PValue::MultipleFloatingTime(u32::from_le_bytes([
                data[0], data[1], data[2], data[3],
            ])),
            PType::MultipleInteger64 => {
                PValue::MultipleInteger64(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleString => {
                PValue::MultipleString(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleString8 => {
                PValue::MultipleString8(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleTime => {
                PValue::MultipleTime(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleGuid => {
                PValue::MultipleGuid(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::MultipleBinary => {
                PValue::MultipleBinary(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Unspecified => {
                PValue::Unspecified(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
            }
            PType::Null => PValue::Null,
            PType::Object => PValue::Object,
        }
    }
}

fn parse_fixed_length_property_entry(data_slice: [u8; 16]) -> FixedLengthPropertyEntry {
    let property_type = PType::from_bits(u16::from_le_bytes([data_slice[0], data_slice[1]]));
    let pid_u16 = u16::from_le_bytes([data_slice[2], data_slice[3]]);
    let property_id = Pid::from_u16(pid_u16);
    let flags = Flags::from_bits(u32::from_le_bytes([
        data_slice[4],
        data_slice[5],
        data_slice[6],
        data_slice[7],
    ]))
    .unwrap();
    let value: [u8; 8] = [
        data_slice[8],
        data_slice[9],
        data_slice[10],
        data_slice[11],
        data_slice[12],
        data_slice[13],
        data_slice[14],
        data_slice[15],
    ];
    let mut flags_string = String::with_capacity(3);
    if flags.contains(Flags::PROPATTR_MANDATORY) {
        flags_string.push('M');
    } else {
        flags_string.push(' ');
    }
    if flags.contains(Flags::PROPATTR_READABLE) {
        flags_string.push('R');
    } else {
        flags_string.push(' ');
    }
    if flags.contains(Flags::PROPATTR_WRITABLE) {
        flags_string.push('W');
    } else {
        flags_string.push(' ');
    }
    eprint!(
        "property_id: 0x{:04X}:{:<24}",
        pid_u16,
        format!("{property_id:?}")
    );
    eprint!(" property_tag: {:<24}", format!("{property_type:?}"));
    eprint!(" flags: {flags_string}");
    // eprint!(
    //     " value: 0x{:02X}{:02X}{:02X}{:02X}",
    //     value[3], value[2], value[1], value[0]
    // );
    let value = PValue::from_bytes(property_type, value);
    eprint!(" value: {value:?}",);

    eprintln!();
    FixedLengthPropertyEntry {
        property_id,
        flags,
        value,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PType {
    Integer16,
    Integer32,
    Floating32,
    Floating64,
    Currency,
    FloatingTime,
    ErrorCode,
    Boolean,
    Integer64,
    String,
    String8,
    Time,
    Guid,
    ServerId,
    Restriction,
    RuleAction,
    Binary,
    MultipleInteger16,
    MultipleInteger32,
    MultipleFloating32,
    MultipleFloating64,
    MultipleCurrency,
    MultipleFloatingTime,
    MultipleInteger64,
    MultipleString,
    MultipleString8,
    MultipleTime,
    MultipleGuid,
    MultipleBinary,
    Unspecified,
    Null,
    Object,
}

impl PType {
    pub fn from_bits(bits: u16) -> Self {
        match bits {
            0x0002 => Self::Integer16,
            0x0003 => Self::Integer32,
            0x0004 => Self::Floating32,
            0x0005 => Self::Floating64,
            0x0006 => Self::Currency,
            0x0007 => Self::FloatingTime,
            0x000A => Self::ErrorCode,
            0x000B => Self::Boolean,
            0x0014 => Self::Integer64,
            0x001F => Self::String,
            0x001E => Self::String8,
            0x0040 => Self::Time,
            0x0048 => Self::Guid,
            0x00FB => Self::ServerId,
            0x00FD => Self::Restriction,
            0x00FE => Self::RuleAction,
            0x0102 => Self::Binary,
            0x1002 => Self::MultipleInteger16,
            0x1003 => Self::MultipleInteger32,
            0x1004 => Self::MultipleFloating32,
            0x1005 => Self::MultipleFloating64,
            0x1006 => Self::MultipleCurrency,
            0x1007 => Self::MultipleFloatingTime,
            0x1014 => Self::MultipleInteger64,
            0x101F => Self::MultipleString,
            0x101E => Self::MultipleString8,
            0x1040 => Self::MultipleTime,
            0x1048 => Self::MultipleGuid,
            0x1102 => Self::MultipleBinary,
            0x0000 => Self::Unspecified,
            0x0001 => Self::Null,
            0x000D => Self::Object,
            _ => panic!("invalid ptag"),
        }
    }
    pub fn to_bits(&self) -> u16 {
        match self {
            Self::Integer16 => 0x0002,
            Self::Integer32 => 0x0003,
            Self::Floating32 => 0x0004,
            Self::Floating64 => 0x0005,
            Self::Currency => 0x0006,
            Self::FloatingTime => 0x0007,
            Self::ErrorCode => 0x000A,
            Self::Boolean => 0x000B,
            Self::Integer64 => 0x0014,
            Self::String => 0x001F,
            Self::String8 => 0x001E,
            Self::Time => 0x0040,
            Self::Guid => 0x0048,
            Self::ServerId => 0x00FB,
            Self::Restriction => 0x00FD,
            Self::RuleAction => 0x00FE,
            Self::Binary => 0x0102,
            Self::MultipleInteger16 => 0x1002,
            Self::MultipleInteger32 => 0x1003,
            Self::MultipleFloating32 => 0x1004,
            Self::MultipleFloating64 => 0x1005,
            Self::MultipleCurrency => 0x1006,
            Self::MultipleFloatingTime => 0x1007,
            Self::MultipleInteger64 => 0x1014,
            Self::MultipleString => 0x101F,
            Self::MultipleString8 => 0x101E,
            Self::MultipleTime => 0x1010,
            Self::MultipleGuid => 0x1048,
            Self::MultipleBinary => 0x1102,
            Self::Unspecified => 0x0000,
            Self::Null => 0x0001,
            Self::Object => 0x000D,
        }
    }
}
