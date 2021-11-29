#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::{
    convert::TryInto,
    io::{Read, Seek},
    path::Path,
};
use uuid::Uuid;

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
        Ok(Self {
            // hash,
            subject,
            sender,
            attachments,
            recipients,
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
        let mut file = std::fs::File::open("troublesome_email.msg").unwrap();
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
        let message = Message {
            string_stream,
            guid_stream,
            subject,
            sender,
        };

        // let root_entry = comp.root_entry();
        // // root_entry.

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
                let mut data_slice = data.as_slice();
                let mut n = 0;
                loop {
                    if data_slice.is_empty() {
                        break;
                    }

                    println!("Stream Length: {}", data_slice.len());
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
                    print!("StringEntry[{}]({})", n, length);
                    std::io::stdout().flush().unwrap();
                    if let Ok(recip0) = read(&data_slice[0..length]) {
                        print!(": {}", recip0);
                    }
                    println!();
                    let next_offset = length + length % 4;
                    println!("next_offset: {}", next_offset);
                    if next_offset > data_slice.len() {
                        break;
                    }
                    data_slice = &data_slice[next_offset..];
                    n += 1;
                }
            } else if s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00030102" {
                let mut data_slice = data.as_slice();
                let mut n = 0;
                loop {
                    println!(
                        "{:02X} {:02X} {:02X} {:02X}",
                        data_slice[0], data_slice[1], data_slice[2], data_slice[3]
                    );
                    println!(
                        "{:02X} {:02X} {:02X} {:02X}",
                        data_slice[4], data_slice[5], data_slice[6], data_slice[7]
                    );
                    let property_index = u16::from_le_bytes([data_slice[6], data_slice[7]]);
                    let guid_index =
                        GuidIndex::new(u16::from_le_bytes([data_slice[4], data_slice[5]]) >> 1);
                    let property_kind: PropertyKind = if data_slice[4] & 0x1 == 1 {
                        PropertyKind::String
                    } else {
                        PropertyKind::Numerical
                    };
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
                            println!("String Index: {}", num);
                            PropertyId::String(message.string_stream.get(num as usize).unwrap())
                        }
                    };
                    print!(
                        "PropertyEntry[{}]: Kind: {:?} Id: {:?} PropertyIndex: {} GUID Index: {:?}",
                        n, property_kind, identifier, property_index, guid_index
                    );
                    std::io::stdout().flush().unwrap();
                    if let GuidIndex::StreamIndex(index) = guid_index {
                        let guid = message.guid_stream.get(index as usize);
                        print!(" GUID: {:?}", guid);
                    }
                    std::io::stdout().flush().unwrap();
                    println!();
                    let stream_id = match identifier {
                        PropertyId::Number(n) => {
                            0x1000 + ((n as u16) ^ (guid_index.as_num() << 1)) % 0x1F
                        }
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
                    println!("stream_name: {}", stream_name);
                    data_slice = &data_slice[8..];
                    if data_slice.is_empty() {
                        break;
                    }
                    n += 1;
                }
            } else if s.path().as_os_str() == "/__nameid_version1.0\\__substg1.0_00020102" {
                let mut data_slice = data.as_slice();
                let mut n = 0;
                loop {
                    if data_slice.is_empty() {
                        break;
                    }
                    let guid: Uuid = parse_guid(data_slice);
                    println!("GUID[{}]: {}", n, guid);
                    data_slice = &data_slice[16..];
                    n += 1;
                }
            } else if s.path().as_os_str() == "/__attach_version1.0_#00000000\\__substg1.0_3001001F"
            {
                print!("Stream[{}]({})", i, data.len());
                if let Ok(recip0) = read(&data) {
                    print!(": ATTACHMENT: {}", recip0);
                }
                println!();
            } else {
                print!("Stream[{}]({})", i, data.len());
                if let Ok(recip0) = read(&data) {
                    print!(": {}", recip0);
                }
                println!();
            }
            // s.
        }
        println!("Subject: {}", message.subject);
        println!("Sender: {}", message.sender);

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
