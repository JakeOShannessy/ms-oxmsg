use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertySet {
    PublicStrings,
    Common,
    Address,
    Headers,
    Appointment,
    Meeting,
    Log,
    Messaging,
    Note,
    PostRss,
    Task,
    UnifiedMessaging,
    PsMapi,
    AirSync,
    Sharing,
    XmlExtrEntities,
    Attachment,
    CalendarAssistant,
    Other(Uuid),
}

const PUBLIC_STRINGS: Uuid = Uuid::from_bytes([
    0x00, 0x02, 0x03, 0x29, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const COMMON: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x08, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const ADDRESS: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x04, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const HEADERS: Uuid = Uuid::from_bytes([
    0x00, 0x02, 0x03, 0x86, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const APPOINTMENT: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x02, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const MEETING: Uuid = Uuid::from_bytes([
    0x6E, 0xD8, 0xDA, 0x90, 0x45, 0x0B, 0x10, 0x1B, 0x98, 0xDA, 0x00, 0xAA, 0x00, 0x3F, 0x13, 0x05,
]);
const LOG: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x0A, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const MESSAGING: Uuid = Uuid::from_bytes([
    0x41, 0xF2, 0x8F, 0x13, 0x83, 0xF4, 0x41, 0x14, 0xA5, 0x84, 0xEE, 0xDB, 0x5A, 0x6B, 0x0B, 0xFF,
]);
const NOTE: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x0E, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const POST_RSS: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x41, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const TASK: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x03, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const UNIFIED_MESSAGING: Uuid = Uuid::from_bytes([
    0x44, 0x42, 0x85, 0x8E, 0xA9, 0xE3, 0x4E, 0x80, 0xB9, 0x00, 0x31, 0x7A, 0x21, 0x0C, 0xC1, 0x5B,
]);
const PS_MAPI: Uuid = Uuid::from_bytes([
    0x00, 0x02, 0x03, 0x28, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const AIR_SYNC: Uuid = Uuid::from_bytes([
    0x71, 0x03, 0x55, 0x49, 0x07, 0x39, 0x4D, 0xCB, 0x91, 0x63, 0x00, 0xF0, 0x58, 0x0D, 0xBB, 0xDF,
]);
const SHARING: Uuid = Uuid::from_bytes([
    0x00, 0x06, 0x20, 0x40, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
const XML_EXTR_ENTITIES: Uuid = Uuid::from_bytes([
    0x23, 0x23, 0x96, 0x08, 0x68, 0x5D, 0x47, 0x32, 0x9C, 0x55, 0x4C, 0x95, 0xCB, 0x4E, 0x8E, 0x33,
]);
const ATTACHMENT: Uuid = Uuid::from_bytes([
    0x96, 0x35, 0x7F, 0x7F, 0x59, 0xE1, 0x47, 0xD0, 0x99, 0xA7, 0x46, 0x51, 0x5C, 0x18, 0x3B, 0x54,
]);
const CALENDAR_ASSISTANT: Uuid = Uuid::from_bytes([
    0x11, 0x00, 0x0E, 0x07, 0xB5, 0x1B, 0x40, 0xD6, 0xAF, 0x21, 0xCA, 0xA8, 0x5E, 0xDA, 0xB1, 0xD0,
]);

impl PropertySet {
    pub fn to_uuid(&self) -> Uuid {
        match self {
            Self::PublicStrings => PUBLIC_STRINGS,
            Self::Common => COMMON,
            Self::Address => ADDRESS,
            Self::Headers => HEADERS,
            Self::Appointment => APPOINTMENT,
            Self::Meeting => MEETING,
            Self::Log => LOG,
            Self::Messaging => MESSAGING,
            Self::Note => NOTE,
            Self::PostRss => POST_RSS,
            Self::Task => TASK,
            Self::UnifiedMessaging => UNIFIED_MESSAGING,
            Self::PsMapi => PS_MAPI,
            Self::AirSync => AIR_SYNC,
            Self::Sharing => SHARING,
            Self::XmlExtrEntities => XML_EXTR_ENTITIES,
            Self::Attachment => ATTACHMENT,
            Self::CalendarAssistant => CALENDAR_ASSISTANT,
            Self::Other(uuid) => *uuid,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Self::PublicStrings => "PS_PUBLIC_STRINGS",
            Self::Common => "PSETID_Common",
            Self::Address => "PSETID_Address",
            Self::Headers => "PS_INTERNET_HEADERS",
            Self::Appointment => "PSETID_Appointment",
            Self::Meeting => "PSETID_Meeting",
            Self::Log => "PSETID_Log",
            Self::Messaging => "PSETID_Messaging",
            Self::Note => "PSETID_Note",
            Self::PostRss => "PSETID_PostRss",
            Self::Task => "PSETID_Task",
            Self::UnifiedMessaging => "PSETID_UnifiedMessaging",
            Self::PsMapi => "PS_MAPI",
            Self::AirSync => "PSETID_AirSync",
            Self::Sharing => "PSETID_Sharing",
            Self::XmlExtrEntities => "PSETID_XmlExtractedEntities",
            Self::Attachment => "PSETID_Attachment",
            Self::CalendarAssistant => "PSETID_Attachment",
            Self::Other(_) => "Other",
        }
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        match uuid {
            PUBLIC_STRINGS => Self::PublicStrings,
            COMMON => Self::Common,
            ADDRESS => Self::Address,
            HEADERS => Self::Headers,
            APPOINTMENT => Self::Appointment,
            MEETING => Self::Meeting,
            LOG => Self::Log,
            MESSAGING => Self::Messaging,
            NOTE => Self::Note,
            POST_RSS => Self::PostRss,
            TASK => Self::Task,
            UNIFIED_MESSAGING => Self::UnifiedMessaging,
            PS_MAPI => Self::PsMapi,
            AIR_SYNC => Self::AirSync,
            SHARING => Self::Sharing,
            XML_EXTR_ENTITIES => Self::XmlExtrEntities,
            ATTACHMENT => Self::Attachment,
            CALENDAR_ASSISTANT => Self::CalendarAssistant,
            uuid => Self::Other(uuid),
        }
    }
}
