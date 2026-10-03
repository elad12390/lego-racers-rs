//! Original SRF UTF-16 strings: u16 count/word count, u16 offsets, text words.
pub struct StringTable {
  pub strings: Vec<String>,
}
impl StringTable {
  pub fn parse(data: &[u8]) -> Result<Self, String> {
    let word = |at: usize| {
      data
        .get(at..at + 2)
        .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
        .ok_or("truncated SRF")
    };
    let count = word(0)? as usize;
    let size = word(2)? as usize;
    let start = 4 + count * 2;
    if data.len() != start + size * 2 {
      return Err("SRF text size mismatch".into());
    }
    let text = data[start..]
      .chunks_exact(2)
      .map(|w| u16::from_le_bytes(w.try_into().unwrap()))
      .collect::<Vec<_>>();
    let mut strings = Vec::with_capacity(count);
    for index in 0..count {
      let offset = word(4 + index * 2)? as usize;
      let suffix = text.get(offset..).ok_or("SRF offset outside text")?;
      let end = suffix
        .iter()
        .position(|c| *c == 0)
        .ok_or("unterminated SRF string")?;
      strings.push(String::from_utf16(&suffix[..end]).map_err(|e| e.to_string())?);
    }
    Ok(Self { strings })
  }
  pub fn get(&self, index: usize) -> Result<&str, String> {
    self
      .strings
      .get(index)
      .map(String::as_str)
      .ok_or_else(|| format!("missing original string {index}"))
  }
}
