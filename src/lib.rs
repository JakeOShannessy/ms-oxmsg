#![allow(dead_code)]
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    convert::TryInto,
    io::{Read, Seek, Write},
    path::Path,
};
use uuid::Uuid;
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
            if property.property_id == 0x0E06 {
                if let Some(PValue::PtypTime(time)) = property.value {
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
        uuid::Uuid::from_u128(u128::from_le_bytes(bytes.try_into().unwrap()))
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

#[cfg(test)]
mod tests {
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

        // let mut addressed_paths = HashSet::new();

        // let mut attachments = Vec::new();

        // for name in AttachmentNameIter::new() {
        //     print!("Looking for attachment stream {} ", name);
        //     if comp.exists(&name) {
        //         println!("Found");
        //         let attachment = Attachment::from_cfb(&mut comp, name);
        //         attachments.push(attachment);
        //     } else {
        //         println!("Not Found");
        //         break;
        //     }
        // }

        // for (i, attachment) in attachments.into_iter().enumerate() {
        //     println!(
        //         "Attachment[{}] ({:?} bytes): {}",
        //         i,
        //         attachment.data.map(|x|x.len),
        //         attachment.name
        //     );
        // }

        // let mut recipients = Vec::new();

        // for name in RecipientNameIter::new() {
        //     print!("Looking for recipient stream {} ", name);
        //     if comp.exists(&name) {
        //         println!("Found");
        //         let recipient = Recipient::from_cfb(&mut comp, name);
        //         recipients.push(recipient);
        //     } else {
        //         println!("Not Found");
        //         break;
        //     }
        // }

        // for (i, recipient) in recipients.into_iter().enumerate() {
        //     println!("Recipients[{}]: {}", i, recipient.address);
        // }
        for e in comp.walk() {
            println!("entry[{}]: {:?}", e.is_storage(), e.path());
            // e.
        }

        let string_stream = {
            let mut stream = comp
                .open_stream("/__nameid_version1.0\\__substg1.0_00040102")
                .unwrap();
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            StringStream { buffer }
        };
        let guid_stream = {
            let mut stream = comp
                .open_stream("/__nameid_version1.0\\__substg1.0_00020102")
                .unwrap();
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            GuidStream { buffer }
        };
        let subject = {
            let mut stream = comp.open_stream("/__substg1.0_0037001F").unwrap();
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            read(&buffer).unwrap()
        };
        let sender = {
            // let mut stream = comp.open_stream("/__substg1.0_3FFA001F")?;
            let mut stream = comp.open_stream("/__substg1.0_0C1F001F").unwrap();
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            read(&buffer).unwrap()
        };
        let body = {
            // let mut stream = comp.open_stream("/__substg1.0_3FFA001F")?;
            let mut stream = comp.open_stream("/__substg1.0_1000001F").unwrap();
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            read(&buffer).ok()
        };
        let properties = {
            let mut stream = comp.open_stream("/__properties_version1.0").unwrap();
            let buffer = {
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).unwrap();
                buffer
            };
            parse_property_stream_top_level(&buffer)
        };
        let mut delivery_time = None;
        for property in properties {
            if property.property_id == 0x0E06 {
                if let Some(PValue::PtypTime(time)) = property.value {
                    delivery_time = Some(time);
                }
            }
        }
        let delivery_time = delivery_time.unwrap();
        let message = Message {
            string_stream,
            guid_stream,
            subject,
            sender,
            delivery_time,
            // body,
        };

        // let root_entry = comp.root_entry();
        // // root_entry.

        #[allow(clippy::needless_collect)]
        let streams: Vec<Entry> = comp.walk_storage("").unwrap().collect();
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
            if s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00040102" {
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
            } else if s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00030102" {
                let len = data.len();
                println!("EntryStream, len = {len}");
                let entry = parse_property(&message, data.as_slice());
            } else if s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00020102" {
                let len = data.len();
                println!("GuidStream, len = {len}");
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
            } else if s.path().as_os_str() == "/__attach_version1.0_#00000000\\__substg1.0_3001001F"
            {
                print!("Stream[{}]({})[{}]", i, data.len(), s.path().display());
                if let Ok(recip0) = read(&data) {
                    print!(": ATTACHMENT: {}", recip0);
                }
                println!();
            } else if s.path().starts_with("/__nameid_version1.0") {
                let len = data.len();
                let name = s.name();
                // println!("named property mapping (len = {len}): {name} - {identifier:?} - {index_kind:?}");
                println!("NamedPropertyMapping (len = {len}): {name}");
                let mut data_slice = data.as_slice();
                let mut n = 0;
                loop {
                    if data_slice.is_empty() {
                        break;
                    }
                    // let guid: Uuid = parse_guid(data_slice);
                    let id_num = u32::from_le_bytes([
                        data_slice[0],
                        data_slice[1],
                        data_slice[2],
                        data_slice[3],
                    ]);
                    let kind_num = parse_kind_index([
                        data_slice[4],
                        data_slice[5],
                        data_slice[6],
                        data_slice[7],
                    ]);
                    println!(
                        "    [{n}]: id/crc: {:?}/{id_num} index/kind: {:?}/{kind_num:?}",
                        &data_slice[0..4],
                        &data_slice[4..8]
                    );
                    match kind_num {
                        (property_index, guid_index, PropertyKind::String) => {
                            // let name = message.string_stream.get(property_index as usize).unwrap();
                            let name = "test";
                            println!("    [{n}]: name: {name} id_num: {id_num} {kind_num:?}",);
                        }
                        (property_index, guid_index, PropertyKind::Numerical) => {
                            println!(
                                "    [{n}]: index: {property_index} id_num: {id_num} {kind_num:?}",
                            );
                        }
                    }
                    data_slice = &data_slice[8..];
                    n += 1;
                }
            } else if s.path().as_os_str() == "/__properties_version1.0" {
                println!("other properties");
                parse_property_stream_top_level(&data);
            } else {
                print!("Stream[{}]({})[{}]", i, data.len(), s.path().display());
                if let Ok(recip0) = read(&data) {
                    print!(": {}", recip0);
                }
                println!();
            }
            // s.
        }
        println!("Subject: {}", message.subject);
        println!("Sender: {}", message.sender);
        if let Some(body) = body {
            println!("Body: {}", body);
        }
        println!("Delivery Time: {}", message.delivery_time);

        // // Append that data to the end of another stream in the same file.
        // {
        //     let mut stream = comp.open_stream("/baz")?;
        //     stream.seek(SeekFrom::End(0))?;
        //     stream.write_all(&data)?;
        // }

        // // Now create a new compound file, and create a new stream with the data.
        // let mut comp2 = cfb::create("some/other/path")?;
        // comp2.create_storage("/spam/")?;
        // let mut stream = comp2.create_stream("/spam/eggs")?;
        // stream.write_all(&data)?;
        let PUBLIC_STRINGS: Uuid = "00020329-0000-0000-C000-000000000046".parse().unwrap();
        let COMMON: Uuid = "00062008-0000-0000-C000-000000000046".parse().unwrap();
        let ADDRESS: Uuid = "00062004-0000-0000-C000-000000000046".parse().unwrap();
        let HEADERS: Uuid = "00020386-0000-0000-C000-000000000046".parse().unwrap();
        let APPOINTMENT: Uuid = "00062002-0000-0000-C000-000000000046".parse().unwrap();
        let MEETING: Uuid = "6ED8DA90-450B-101B-98DA-00AA003F1305".parse().unwrap();
        let LOG: Uuid = "0006200A-0000-0000-C000-000000000046".parse().unwrap();
        let MESSAGING: Uuid = "41F28F13-83F4-4114-A584-EEDB5A6B0BFF".parse().unwrap();
        let NOTE: Uuid = "0006200E-0000-0000-C000-000000000046".parse().unwrap();
        let POST_RSS: Uuid = "00062041-0000-0000-C000-000000000046".parse().unwrap();
        let TASK: Uuid = "00062003-0000-0000-C000-000000000046".parse().unwrap();
        let UNIFIED_MESSAGING: Uuid = "4442858E-A9E3-4E80-B900-317A210CC15B".parse().unwrap();
        let PS_MAPI: Uuid = "00020328-0000-0000-C000-000000000046".parse().unwrap();
        let AIR_SYNC: Uuid = "71035549-0739-4DCB-9163-00F0580DBBDF".parse().unwrap();
        let SHARING: Uuid = "00062040-0000-0000-C000-000000000046".parse().unwrap();
        let XML_EXTR_ENTITIES: Uuid = "23239608-685D-4732-9C55-4C95CB4E8E33".parse().unwrap();
        let ATTACHMENT: Uuid = "96357F7F-59E1-47D0-99A7-46515C183B54".parse().unwrap();
        let CALENDAR_ASSISTANT: Uuid = "11000E07-B51B-40D6-AF21-CAA85EDAB1D0".parse().unwrap();
        assert_eq!(
            PUBLIC_STRINGS,
            Uuid::from_bytes([
                0x00, 0x02, 0x03, 0x29, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x46
            ])
        );
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

fn parse_property(message: &Message, data_slice: &[u8]) {
    let mut data_slice = data_slice;
    let mut n = 0;
    loop {
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
                let num = u32::from_le_bytes([
                    data_slice[0],
                    data_slice[1],
                    data_slice[2],
                    data_slice[3],
                ]);
                // println!("        String Index: {}", num);
                PropertyId::String(message.string_stream.get(num as usize).unwrap())
            }
        };
        println!("    PropertyEntry[{n}][{property_index}]: Id: {identifier:?} PropertyIndex: {property_index} GuidIndex: {guid_index:?}");
        println!(
            "        {:02X} {:02X} {:02X} {:02X}",
            data_slice[0], data_slice[1], data_slice[2], data_slice[3]
        );
        println!(
            "        {:02X} {:02X} {:02X} {:02X}",
            data_slice[4], data_slice[5], data_slice[6], data_slice[7]
        );
        std::io::stdout().flush().unwrap();
        if let GuidIndex::StreamIndex(index) = guid_index {
            let guid = message.guid_stream.get(index as usize);
            print!("        GUID: {:?}", guid);
        }
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
        data_slice = &data_slice[8..];
        if data_slice.is_empty() {
            break;
        }
        n += 1;
    }
}

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
        // let property_tag = &data_slice[0..4];
        let property_tag = u16::from_le_bytes([data_slice[0], data_slice[1]]);
        let property_id = u16::from_le_bytes([data_slice[2], data_slice[3]]);
        let flags =
            u32::from_le_bytes([data_slice[4], data_slice[5], data_slice[6], data_slice[7]]);
        let value = &data_slice[8..16];
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
    struct Flags: u32 {
        const PROPATTR_MANDATORY = 0x00000001;
        const PROPATTR_READABLE = 0x00000002;
        const PROPATTR_WRITABLE = 0x00000004;
    }
}

pub struct FixedLengthPropertyEntry {
    pub property_id: u16,
    pub value: Option<PValue>,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum PValue {
    PtypInteger16(i16),
    PtypInteger32(i32),
    PtypFloating32(f32),
    PtypFloating64(f64),
    PtypCurrency(u64),
    PtypFloatingTime,
    PtypErrorCode,
    PtypBoolean(bool),
    PtypInteger64(i64),
    PtypString,
    PtypString8,
    PtypTime(DateTime<Utc>),
    PtypGuid,
    PtypServerId,
    PtypRestriction,
    PtypRuleAction,
    PtypBinary,
    PtypMultipleInteger16,
    PtypMultipleInteger32,
    PtypMultipleFloating32,
    PtypMultipleFloating64,
    PtypMultipleCurrency,
    PtypMultipleFloatingTime,
    PtypMultipleInteger64,
    PtypMultipleString,
    PtypMultipleString8,
    PtypMultipleTime,
    PtypMultipleGuid,
    PtypMultipleBinary,
    PtypUnspecified,
    PtypNull,
    PtypObject,
}

fn parse_fixed_length_property_entry(data_slice: [u8; 16]) -> FixedLengthPropertyEntry {
    let property_tag = PTag::from_bits(u16::from_le_bytes([data_slice[0], data_slice[1]]));
    let property_id = u16::from_le_bytes([data_slice[2], data_slice[3]]);
    let flags = Flags::from_bits(u32::from_le_bytes([
        data_slice[4],
        data_slice[5],
        data_slice[6],
        data_slice[7],
    ]))
    .unwrap();
    let value = &data_slice[8..16];
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
    eprint!("property_id: 0x{:04X}", property_id);
    eprint!(" property_tag: {:<24}", format!("{property_tag:?}"));
    eprint!(" flags: {flags_string}");
    eprint!(
        " value: 0x{:02X}{:02X}{:02X}{:02X}",
        value[3], value[2], value[1], value[0]
    );
    let value = match property_tag {
        PTag::PtypTime => {
            // parse time
            let nano_100s = i64::from_le_bytes([
                value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
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
            Some(PValue::PtypTime(utc_time))
        }
        _ => None,
    };
    eprintln!();
    FixedLengthPropertyEntry { property_id, value }
}

fn parse_fixed_length_pv(data_slice: [u8; 8]) {
    // let data = [data_slice[0], data_slice[1], data_slice[2], data_slice[3]];
    // let _reserved = [data_slice[4], data_slice[5], data_slice[6], data_slice[7]];
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PTag {
    PtypInteger16,
    PtypInteger32,
    PtypFloating32,
    PtypFloating64,
    PtypCurrency,
    PtypFloatingTime,
    PtypErrorCode,
    PtypBoolean,
    PtypInteger64,
    PtypString,
    PtypString8,
    PtypTime,
    PtypGuid,
    PtypServerId,
    PtypRestriction,
    PtypRuleAction,
    PtypBinary,
    PtypMultipleInteger16,
    PtypMultipleInteger32,
    PtypMultipleFloating32,
    PtypMultipleFloating64,
    PtypMultipleCurrency,
    PtypMultipleFloatingTime,
    PtypMultipleInteger64,
    PtypMultipleString,
    PtypMultipleString8,
    PtypMultipleTime,
    PtypMultipleGuid,
    PtypMultipleBinary,
    PtypUnspecified,
    PtypNull,
    PtypObject,
}

impl PTag {
    pub fn from_bits(bits: u16) -> Self {
        match bits {
            0x0002 => Self::PtypInteger16,
            0x0003 => Self::PtypInteger32,
            0x0004 => Self::PtypFloating32,
            0x0005 => Self::PtypFloating64,
            0x0006 => Self::PtypCurrency,
            0x0007 => Self::PtypFloatingTime,
            0x000A => Self::PtypErrorCode,
            0x000B => Self::PtypBoolean,
            0x0014 => Self::PtypInteger64,
            0x001F => Self::PtypString,
            0x001E => Self::PtypString8,
            0x0040 => Self::PtypTime,
            0x0048 => Self::PtypGuid,
            0x00FB => Self::PtypServerId,
            0x00FD => Self::PtypRestriction,
            0x00FE => Self::PtypRuleAction,
            0x0102 => Self::PtypBinary,
            0x1002 => Self::PtypMultipleInteger16,
            0x1003 => Self::PtypMultipleInteger32,
            0x1004 => Self::PtypMultipleFloating32,
            0x1005 => Self::PtypMultipleFloating64,
            0x1006 => Self::PtypMultipleCurrency,
            0x1007 => Self::PtypMultipleFloatingTime,
            0x1014 => Self::PtypMultipleInteger64,
            0x101F => Self::PtypMultipleString,
            0x101E => Self::PtypMultipleString8,
            0x1010 => Self::PtypMultipleTime,
            0x1048 => Self::PtypMultipleGuid,
            0x1102 => Self::PtypMultipleBinary,
            0x0000 => Self::PtypUnspecified,
            0x0001 => Self::PtypNull,
            0x000D => Self::PtypObject,
            _ => panic!("invalid ptag"),
        }
    }
    pub fn to_bits(&self) -> u16 {
        match self {
            Self::PtypInteger16 => 0x0002,
            Self::PtypInteger32 => 0x0003,
            Self::PtypFloating32 => 0x0004,
            Self::PtypFloating64 => 0x0005,
            Self::PtypCurrency => 0x0006,
            Self::PtypFloatingTime => 0x0007,
            Self::PtypErrorCode => 0x000A,
            Self::PtypBoolean => 0x000B,
            Self::PtypInteger64 => 0x0014,
            Self::PtypString => 0x001F,
            Self::PtypString8 => 0x001E,
            Self::PtypTime => 0x0040,
            Self::PtypGuid => 0x0048,
            Self::PtypServerId => 0x00FB,
            Self::PtypRestriction => 0x00FD,
            Self::PtypRuleAction => 0x00FE,
            Self::PtypBinary => 0x0102,
            Self::PtypMultipleInteger16 => 0x1002,
            Self::PtypMultipleInteger32 => 0x1003,
            Self::PtypMultipleFloating32 => 0x1004,
            Self::PtypMultipleFloating64 => 0x1005,
            Self::PtypMultipleCurrency => 0x1006,
            Self::PtypMultipleFloatingTime => 0x1007,
            Self::PtypMultipleInteger64 => 0x1014,
            Self::PtypMultipleString => 0x101F,
            Self::PtypMultipleString8 => 0x101E,
            Self::PtypMultipleTime => 0x1010,
            Self::PtypMultipleGuid => 0x1048,
            Self::PtypMultipleBinary => 0x1102,
            Self::PtypUnspecified => 0x0000,
            Self::PtypNull => 0x0001,
            Self::PtypObject => 0x000D,
        }
    }
}
