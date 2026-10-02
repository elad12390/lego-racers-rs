use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

const MAGIC: &[u8; 4] = b"LJAM";
const GROUP_NAME_LEN: usize = 8;
const GROUP_ENTRY_LEN: usize = 16;
const FILE_NAME_LEN: usize = 12;
const FILE_ENTRY_LEN: usize = 20;

#[derive(Debug)]
pub enum JamError {
    Io(io::Error),
    BadMagic,
    OutOfBounds { what: &'static str, offset: usize },
}

impl fmt::Display for JamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JamError::Io(error) => write!(f, "io error: {error}"),
            JamError::BadMagic => write!(f, "not a LEGO.JAM archive (missing LJAM magic)"),
            JamError::OutOfBounds { what, offset } => {
                write!(f, "{what} at offset {offset:#x} runs past the end of the archive")
            }
        }
    }
}

impl std::error::Error for JamError {}

impl From<io::Error> for JamError {
    fn from(error: io::Error) -> Self {
        JamError::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub group: String,
    pub name: String,
    pub entries: Vec<Entry>,
}

pub struct Jam {
    data: Vec<u8>,
    pub tables: Vec<Table>,
}

impl Jam {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JamError> {
        Self::parse(fs::read(path)?)
    }

    pub fn parse(data: Vec<u8>) -> Result<Self, JamError> {
        if data.get(..4) != Some(MAGIC.as_slice()) {
            return Err(JamError::BadMagic);
        }
        let mut tables = Vec::new();
        let group_count = read_u32(&data, 8, "group count")? as usize;
        for index in 0..group_count {
            let at = 12 + index * GROUP_ENTRY_LEN;
            let group = read_name(&data, at, GROUP_NAME_LEN, "group name")?;
            let offset = read_u32(&data, at + GROUP_NAME_LEN + 4, "group offset")? as usize;
            read_group(&data, &group, offset, &mut tables)?;
        }
        Ok(Jam { data, tables })
    }

    pub fn bytes(&self, entry: &Entry) -> Result<&[u8], JamError> {
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        self.data.get(start..end).ok_or(JamError::OutOfBounds {
            what: "file data",
            offset: start,
        })
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

fn read_group(
    data: &[u8],
    group: &str,
    offset: usize,
    tables: &mut Vec<Table>,
) -> Result<(), JamError> {
    read_directory(data,group,group,offset,tables,&mut Vec::new())
}

fn read_directory(data:&[u8],group:&str,name:&str,offset:usize,tables:&mut Vec<Table>,ancestors:&mut Vec<usize>)->Result<(),JamError> {
    if ancestors.contains(&offset)||ancestors.len()>=64 {return Err(JamError::OutOfBounds {what:"cyclic/deep directory",offset});}
    ancestors.push(offset);
    let files=read_u32(data,offset,"file count")? as usize;
    if files>(data.len().saturating_sub(offset+4))/FILE_ENTRY_LEN {return Err(JamError::OutOfBounds {what:"file table",offset});}
    if files>0 {tables.push(read_table(data,group,name,offset)?);}
    // Directories may contain BOTH files and child directories. The original
    // MENUDATA folder has138files followed by33children, including PIECEDB.
    let directory_offset=offset+4+files*FILE_ENTRY_LEN;
    let count = read_u32(data, directory_offset, "directory count")? as usize;
    if count>(data.len().saturating_sub(directory_offset+4))/GROUP_ENTRY_LEN {return Err(JamError::OutOfBounds {what:"directory table",offset:directory_offset});}
    for index in 0..count {
        let at = directory_offset + 4 + index * GROUP_ENTRY_LEN;
        let name = read_name(data, at, GROUP_NAME_LEN, "table name")?;
        let table_offset = read_u32(data, at + GROUP_NAME_LEN + 4, "table offset")? as usize;
        read_directory(data,group,&name,table_offset,tables,ancestors)?;
    }
    ancestors.pop();
    Ok(())
}

fn read_table(data: &[u8], group: &str, name: &str, offset: usize) -> Result<Table, JamError> {
    let count = read_u32(data, offset, "file count")? as usize;
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let at = offset + 4 + index * FILE_ENTRY_LEN;
        entries.push(Entry {
            name: read_name(data, at, FILE_NAME_LEN, "file name")?,
            offset: read_u32(data, at + FILE_NAME_LEN, "file offset")?,
            size: read_u32(data, at + FILE_NAME_LEN + 4, "file size")?,
        });
    }
    Ok(Table {
        group: group.to_string(),
        name: name.to_string(),
        entries,
    })
}

fn read_u32(data: &[u8], offset: usize, what: &'static str) -> Result<u32, JamError> {
    data.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .ok_or(JamError::OutOfBounds { what, offset })
}

fn read_name(data: &[u8], offset: usize, len: usize, what: &'static str) -> Result<String, JamError> {
    let bytes = data
        .get(offset..offset + len)
        .ok_or(JamError::OutOfBounds { what, offset })?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}
