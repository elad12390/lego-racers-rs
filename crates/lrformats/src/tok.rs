use std::collections::HashMap;
use std::fmt;

const STR: u8 = 2;
const F32: u8 = 3;
const I32: u8 = 4;
const OPEN: u8 = 5;
const CLOSE: u8 = 6;
const LBRACKET: u8 = 7;
const RBRACKET: u8 = 8;
const COMMA: u8 = 9;
const SEMI: u8 = 10;
const PACKED: u8 = 0x14;
const LAYOUT: u8 = 0x16;
const FIRST_FIELD_NAME: u8 = 0x20;

#[derive(Debug, PartialEq)]
pub enum TokError {
    Eof,
    UnknownFieldType(u8),
    UnterminatedString,
    BadBracket,
    UndeclaredPackedKind(u8),
}

impl fmt::Display for TokError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokError::Eof => write!(f, "file ended in the middle of a token"),
            TokError::UnknownFieldType(t) => write!(f, "unknown field type {t:#04x}"),
            TokError::UnterminatedString => write!(f, "string without terminator"),
            TokError::BadBracket => write!(f, "malformed [count]"),
            TokError::UndeclaredPackedKind(k) => write!(f, "packed array of undeclared kind {k:#04x}"),
        }
    }
}

impl std::error::Error for TokError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    I32(i32),
    F32(f32),
}

impl Value {
    pub fn as_f32(self) -> Option<f32> {
        match self {
            Value::F32(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_u32(self) -> Option<u32> {
        match self {
            Value::U8(v) => Some(v.into()),
            Value::I8(v) => u32::try_from(v).ok(),
            Value::U16(v) => Some(v.into()),
            Value::I16(v) => u32::try_from(v).ok(),
            Value::I32(v) => u32::try_from(v).ok(),
            Value::F32(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Keyword(u8),
    Str(String),
    Float(f32),
    Int(i32),
    /// `[n]`
    Count(u32),
    /// Inline bracketed values used by original font character maps.
    List(Vec<Node>),
    Comma,
    Semi,
    Block(Vec<Node>),
    /// One record introduced by its kind tag.
    Record { kind: u8, fields: Vec<Value> },
    /// Tagged string-only records used by race archive descriptors (.RAB).
    StringRecord {kind:u8,fields:Vec<String>},
    /// `14 <u16 count> <kind>` followed by `count` records (or primitives) of that kind.
    Packed { kind: u8, rows: Vec<Vec<Value>> },
    PackedStrings(Vec<String>),
}

/// Parses the tokenized container used by the GDB/MDB/TDB/SDB/ADB/WDB files.
///
/// Strict: succeeds only if every byte is consumed by well-formed tokens.
pub fn parse(data: &[u8]) -> Result<Vec<Node>, TokError> {
    let mut parser = Parser { data, pos: 0, layouts: HashMap::new(),repeated:None };
    let mut nodes = Vec::new();
    while parser.pos < data.len()||parser.repeated.is_some() {
        nodes.push(parser.node()?);
    }
    Ok(nodes)
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    layouts: HashMap<u8, Vec<u8>>,
    repeated:Option<(u16,u8)>,
}

impl Parser<'_> {
    // The archive also packs repeated punctuation: ICB_CHAR ends nested
    // material/mesh/model blocks with `14 04 00 06`. These have no payload.
    fn peek_code(&mut self)->Result<u8,TokError> {
        if let Some((_,code))=self.repeated {return Ok(code);}
        let code=*self.data.get(self.pos).ok_or(TokError::Eof)?;
        if code==PACKED {
            let header=self.data.get(self.pos+1..self.pos+4).ok_or(TokError::Eof)?;
            if matches!(header[2],OPEN|CLOSE|LBRACKET|RBRACKET|COMMA|SEMI) {
                let count=u16::from_le_bytes([header[0],header[1]]);
                if count==0 {return Err(TokError::BadBracket);}
                self.pos+=4;self.repeated=Some((count,header[2]));return Ok(header[2]);
            }
        }
        Ok(code)
    }
    fn code(&mut self)->Result<u8,TokError> {
        let code=self.peek_code()?;
        if let Some((count,kind))=self.repeated {self.repeated=if count==1 {None} else {Some((count-1,kind))};}
        else {self.pos+=1;}
        Ok(code)
    }
    fn byte(&mut self) -> Result<u8, TokError> {
        let b = *self.data.get(self.pos).ok_or(TokError::Eof)?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], TokError> {
        let slice = self.data.get(self.pos..self.pos + n).ok_or(TokError::Eof)?;
        self.pos += n;
        Ok(slice)
    }

    fn string(&mut self) -> Result<String, TokError> {
        let rest = &self.data[self.pos..];
        let end = rest.iter().position(|&b| b == 0).ok_or(TokError::UnterminatedString)?;
        let text = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.pos += end + 1;
        Ok(text)
    }

    fn field(&mut self, field_type: u8) -> Result<Option<Value>, TokError> {
        Ok(match field_type {
            F32 => Some(Value::F32(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))),
            I32 => Some(Value::I32(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))),
            0x0b => Some(Value::I8(self.byte()? as i8)),
            0x0c => Some(Value::U8(self.byte()?)),
            0x0d => Some(Value::I16(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))),
            0x0e => Some(Value::U16(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))),
            t if t >= FIRST_FIELD_NAME => None,
            t => return Err(TokError::UnknownFieldType(t)),
        })
    }

    fn record(&mut self, kind: u8) -> Result<Vec<Value>, TokError> {
        let types = self.layouts[&kind].clone();
        let mut fields = Vec::with_capacity(types.len());
        for t in types {
            fields.extend(self.field(t)?);
        }
        Ok(fields)
    }

    fn declare_layout(&mut self) -> Result<(), TokError> {
        let kind = self.byte()?;
        let count = self.byte()?;
        let types: Vec<u8> = (0..count).map(|_| self.byte()).collect::<Result<_, _>>()?;
        for &t in &types {
            if !matches!(t, STR | F32 | I32 | 0x0b | 0x0c | 0x0d | 0x0e) && t < FIRST_FIELD_NAME {
                return Err(TokError::UnknownFieldType(t));
            }
        }
        self.layouts.insert(kind, types);
        Ok(())
    }

    fn packed(&mut self) -> Result<Node, TokError> {
        let count = u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as usize;
        let kind = self.byte()?;
        if self.layouts.contains_key(&kind) {
            let rows = (0..count).map(|_| self.record(kind)).collect::<Result<_, _>>()?;
            return Ok(Node::Packed { kind, rows });
        }
        match kind {
            STR => Ok(Node::PackedStrings(
                (0..count).map(|_| self.string()).collect::<Result<_, _>>()?,
            )),
            F32 | I32 | 0x0b | 0x0c | 0x0d | 0x0e => {
                let mut rows = Vec::with_capacity(count);
                for _ in 0..count {
                    rows.push(self.field(kind)?.into_iter().collect());
                }
                Ok(Node::Packed { kind, rows })
            }
            other => Err(TokError::UndeclaredPackedKind(other)),
        }
    }

    fn block(&mut self) -> Result<Vec<Node>, TokError> {
        let mut nodes = Vec::new();
        loop {
            if self.peek_code()? == CLOSE {
                self.code()?;
                return Ok(nodes);
            }
            if self.pos >= self.data.len() {
                return Err(TokError::Eof);
            }
            if self.data[self.pos] == LAYOUT {
                self.pos += 1;
                self.declare_layout()?;
                continue;
            }
            nodes.push(self.node()?);
        }
    }

    fn node(&mut self) -> Result<Node, TokError> {
        let b = self.code()?;
        Ok(match b {
            OPEN => Node::Block(self.block()?),
            LAYOUT => {
                self.declare_layout()?;
                return self.node();
            }
            PACKED => self.packed()?,
            STR => Node::Str(self.string()?),
            F32 => Node::Float(f32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            I32 => Node::Int(i32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            // TaggedFileStream::ReadTagged normalizes standalone compact
            // integer tokens too, not only fields inside declared records.
            0x0b | 0x0c | 0x0d | 0x0e => Node::Int(match self.field(b)?.unwrap() {
                Value::I8(v)=>v as i32,Value::U8(v)=>v as i32,
                Value::I16(v)=>v as i32,Value::U16(v)=>v as i32,_=>unreachable!(),
            }),
            LBRACKET => {
                let mut values=Vec::new();
                while self.peek_code()?!=RBRACKET {
                    if self.pos>=self.data.len() {return Err(TokError::BadBracket);}
                    values.push(self.node()?);
                }
                self.code()?;
                match values.as_slice() {
                    [Node::Int(n)] if *n>=0=>Node::Count(*n as u32),
                    _=>Node::List(values),
                }
            }
            COMMA => Node::Comma,
            SEMI => Node::Semi,
            kind if self.layouts.contains_key(&kind) => {
                let types=self.layouts[&kind].clone();
                if types.contains(&STR) {
                    let mut fields=Vec::new();
                    for t in types {
                        if t==STR {fields.push(self.string()?);}
                        else if t<FIRST_FIELD_NAME {return Err(TokError::UnknownFieldType(t));}
                    }
                    Node::StringRecord {kind,fields}
                } else {Node::Record {kind,fields:self.record(kind)?}}
            },
            keyword => Node::Keyword(keyword),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_counted_list_of_strings() {
        // 27 [2] { "a" "bc" }
        let data = [0x27, 7, 4, 2, 0, 0, 0, 8, 5, 2, b'a', 0, 2, b'b', b'c', 0, 6];
        let nodes = parse(&data).unwrap();
        assert_eq!(
            nodes,
            vec![
                Node::Keyword(0x27),
                Node::Count(2),
                Node::Block(vec![Node::Str("a".into()), Node::Str("bc".into())]),
            ]
        );
    }

    #[test]
    fn packed_records_follow_the_declared_layout() {
        // layout 0x19 = three bytes; { 14 count=2 kind=0x19 <6 bytes> }
        let data = [0x16, 0x19, 3, 0x0c, 0x0c, 0x0c, 5, 0x14, 2, 0, 0x19, 1, 2, 3, 4, 5, 6, 6];
        let nodes = parse(&data).unwrap();
        let Node::Block(body) = &nodes[0] else { panic!("expected a block") };
        let Node::Packed { kind, rows } = &body[0] else { panic!("expected packed rows") };
        assert_eq!(*kind, 0x19);
        assert_eq!(rows[1], vec![Value::U8(4), Value::U8(5), Value::U8(6)]);
    }

    #[test]
    fn tagged_records_and_name_fields_take_no_bytes() {
        // layout 0x1c has a name field (0x27) and a u16; records are introduced by their tag.
        let data = [0x16, 0x1c, 2, 0x27, 0x0e, 5, 0x1c, 7, 0, 0x1c, 9, 0, 6];
        let nodes = parse(&data).unwrap();
        let Node::Block(body) = &nodes[0] else { panic!("expected a block") };
        assert_eq!(body.len(), 2);
        assert_eq!(body[1], Node::Record { kind: 0x1c, fields: vec![Value::U16(9)] });
    }

    #[test]
    fn packed_strings_and_primitives_need_no_layout() {
        let data = [5, 0x14, 2, 0, 2, b'x', 0, b'y', 0, 0x14, 1, 0, 3, 0, 0, 0x80, 0x3f, 6];
        let nodes = parse(&data).unwrap();
        let Node::Block(body) = &nodes[0] else { panic!("expected a block") };
        assert_eq!(body[0], Node::PackedStrings(vec!["x".into(), "y".into()]));
        assert_eq!(body[1], Node::Packed { kind: 3, rows: vec![vec![Value::F32(1.0)]] });
    }

    #[test]
    fn reports_truncation_and_unknown_types() {
        assert_eq!(parse(&[5, 2, b'a']), Err(TokError::UnterminatedString));
        assert_eq!(parse(&[5]), Err(TokError::Eof));
        assert_eq!(parse(&[0x16, 0x17, 1, 0x10]), Err(TokError::UnknownFieldType(0x10)));
    }

    #[test]
    fn tagged_string_layouts_read_null_terminated_fields_without_tag_bytes() {
        let bytes=[0x16,0x17,3,0x2b,2,2,5,0x17,b'a',0,b'b',b'c',0,6];
        assert_eq!(parse(&bytes).unwrap(),vec![Node::Block(vec![Node::StringRecord {kind:0x17,fields:vec!["a".into(),"bc".into()]}])]);
        assert_eq!(parse(&bytes[..bytes.len()-2]),Err(TokError::UnterminatedString));
    }

    #[test]
    fn standalone_compact_integers_do_not_turn_payload_bytes_into_tags() {
        assert_eq!(parse(&[0x0d,0xff,0xff,0x0e,0x34,0x12,0x0c,0x80]).unwrap(),vec![Node::Int(-1),Node::Int(0x1234),Node::Int(128)]);
    }
    #[test]
    fn packed_closing_punctuation_unwinds_nested_model_blocks() {
        assert_eq!(parse(&[5,5,5,0x0c,42,0x14,3,0,6]).unwrap(),vec![Node::Block(vec![Node::Block(vec![Node::Block(vec![Node::Int(42)])])])]);
    }
}
