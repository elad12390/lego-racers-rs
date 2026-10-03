use std::collections::HashMap;
use std::path::Path;

use lrjam::{Jam, JamError};

/// Name lookup over a whole LEGO.JAM, so the game reads its original data file directly.
pub struct Library {
  jam: Jam,
  index: HashMap<String, Vec<(usize, usize)>>,
}

impl Library {
  pub fn open(path: impl AsRef<Path>) -> Result<Self, JamError> {
    Ok(Self::new(Jam::open(path)?))
  }

  pub fn new(jam: Jam) -> Self {
    let mut index: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (t, table) in jam.tables.iter().enumerate() {
      for (e, entry) in table.entries.iter().enumerate() {
        index
          .entry(entry.name.to_ascii_uppercase())
          .or_default()
          .push((t, e));
      }
    }
    Library { jam, index }
  }

  pub fn jam(&self) -> &Jam {
    &self.jam
  }

  /// Exact original group/table ownership, including duplicated ENGLISH names.
  pub fn find_at(&self, name: &str, group: &str, table: &str) -> Option<&[u8]> {
    let hits = self.index.get(&name.to_ascii_uppercase())?;
    let (t, e) = *hits.iter().find(|(t, _)| {
      self.jam.tables[*t].group.eq_ignore_ascii_case(group)
        && self.jam.tables[*t].name.eq_ignore_ascii_case(table)
    })?;
    self.jam.bytes(&self.jam.tables[t].entries[e]).ok()
  }

  /// Finds a file by name (any case). When several tables hold it, the one named `prefer`
  /// wins, then GAMEDATA/COMMON, then the first.
  pub fn find(&self, name: &str, prefer: Option<&str>) -> Option<&[u8]> {
    let hits = self.index.get(&name.to_ascii_uppercase())?;
    let pick = |wanted: &str| {
      hits
        .iter()
        .find(|(t, _)| self.jam.tables[*t].name.eq_ignore_ascii_case(wanted))
    };
    let (t, e) = prefer
      .and_then(pick)
      .or_else(|| pick("COMMON"))
      .or_else(|| hits.first())
      .copied()?;
    self.jam.bytes(&self.jam.tables[t].entries[e]).ok()
  }

  pub fn tables(&self) -> impl Iterator<Item = &str> {
    self.jam.tables.iter().map(|t| t.name.as_str())
  }

  /// Strict owning-table lookup for gameplay data. No cross-race fallback.
  pub fn find_in(&self, name: &str, table: &str) -> Option<&[u8]> {
    let &(t, e) = self
      .index
      .get(&name.to_ascii_uppercase())?
      .iter()
      .find(|(t, _)| self.jam.tables[*t].name.eq_ignore_ascii_case(table))?;
    self.jam.bytes(&self.jam.tables[t].entries[e]).ok()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn jam_with(entries: &[(&str, &str, &[u8])]) -> Library {
    // Build a minimal in-memory archive through the real parser: one group per entry set.
    let mut data = Vec::new();
    let tables: Vec<&str> = {
      let mut t: Vec<&str> = entries.iter().map(|e| e.0).collect();
      t.dedup();
      t
    };
    data.extend_from_slice(b"LJAM\0\0\0\0");
    data.extend_from_slice(&(tables.len() as u32).to_le_bytes());
    let header_len = 12 + tables.len() * 16;
    let mut table_blobs: Vec<Vec<u8>> = Vec::new();
    let mut payload: Vec<u8> = Vec::new();
    let mut offset = header_len;
    let mut blob_sizes = Vec::new();
    for table in &tables {
      let files: Vec<_> = entries.iter().filter(|e| e.0 == *table).collect();
      // Every directory ends with a child-directory count, even leaf tables.
      blob_sizes.push(8 + files.len() * 20);
    }
    let mut data_at = header_len + blob_sizes.iter().sum::<usize>();
    for (i, table) in tables.iter().enumerate() {
      let files: Vec<_> = entries.iter().filter(|e| e.0 == *table).collect();
      let mut blob = (files.len() as u32).to_le_bytes().to_vec();
      for f in files {
        let mut name = f.1.as_bytes().to_vec();
        name.resize(12, 0);
        blob.extend_from_slice(&name);
        blob.extend_from_slice(&(data_at as u32).to_le_bytes());
        blob.extend_from_slice(&(f.2.len() as u32).to_le_bytes());
        payload.extend_from_slice(f.2);
        data_at += f.2.len();
      }
      blob.extend_from_slice(&0u32.to_le_bytes());
      let mut name = table.as_bytes().to_vec();
      name.resize(8, 0);
      data.extend_from_slice(&name);
      data.extend_from_slice(&[0; 4]);
      data.extend_from_slice(&(offset as u32).to_le_bytes());
      offset += blob_sizes[i];
      table_blobs.push(blob);
    }
    for blob in table_blobs {
      data.extend_from_slice(&blob);
    }
    data.extend_from_slice(&payload);
    Library::new(Jam::parse(data).unwrap())
  }

  #[test]
  fn finds_names_case_insensitively_and_prefers_the_requested_table() {
    let lib = jam_with(&[
      ("COMMON", "CAR.BMP", b"common"),
      ("RACE1", "CAR.BMP", b"race1"),
    ]);
    assert_eq!(lib.find("car.bmp", None), Some(&b"common"[..]));
    assert_eq!(lib.find("Car.Bmp", Some("RACE1")), Some(&b"race1"[..]));
    assert_eq!(lib.find("missing.bmp", None), None);
  }
}
