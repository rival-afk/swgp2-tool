//! Minimal, lossless reader/writer for .NET BinaryFormatter (MS-NRBF) streams.
//! Only the record subset needed to round-trip the SWGP2 save file is
//! implemented, but the implementation is generic over those record types.

use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct NrbfError(pub String);

impl fmt::Display for NrbfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for NrbfError {}

type Result<T> = std::result::Result<T, NrbfError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(NrbfError(msg.into()))
}

// ---------------------------------------------------------------------------
// Primitive values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Boolean(bool),
    Byte(u8),
    SByte(i8),
    Char(String),
    Decimal(String),
    Double(f64),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Single(f32),
    TimeSpan(i64),
    DateTime(u64),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Null, // PrimitiveType::Null - carries no payload
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// ObjectNull record (a class member / array element that is null)
    Null,
    /// Index into `Graph::arena`
    Ref(usize),
    /// Raw primitive typed by the owning member/class context
    Prim(Prim),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Additional {
    None,
    Prim(u8),
    SysClass(String),
    Class(String, i32),
}

fn additional_for(binary_type: u8, additional: &Additional, for_array: bool) -> Result<Vec<u8>> {
    let _ = for_array;
    match binary_type {
        // Primitive / PrimitiveArray
        0 | 7 => match additional {
            Additional::Prim(p) => Ok(vec![*p]),
            _ => err("expected primitive additional info"),
        },
        // SystemClass
        3 => match additional {
            Additional::SysClass(s) => {
                let mut out = encode_lps(s);
                out.shrink_to_fit();
                Ok(out)
            }
            _ => err("expected SystemClass type name"),
        },
        // Class
        4 => match additional {
            Additional::Class(name, lib_id) => {
                let mut out = encode_lps(name);
                out.extend_from_slice(&lib_id.to_le_bytes());
                Ok(out)
            }
            _ => err("expected Class type name and library id"),
        },
        _ => Ok(vec![]),
    }
}

fn encode_lps(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() + 5);
    let mut len = bytes.len();
    while len >= 0x80 {
        out.push((len as u8) | 0x80);
        len >>= 7;
    }
    out.push(len as u8);
    out.extend_from_slice(bytes);
    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Str(String),
    Class {
        system: bool,
        name: String,
        library_id: i32,
        member_names: Vec<String>,
        binary_types: Vec<u8>,
        additional: Vec<Additional>,
        members: Vec<Value>,
    },
    ArraySinglePrimitive {
        ptype: u8,
        items: Vec<Prim>,
    },
    ArraySingleString {
        items: Vec<Value>,
    },
    ArraySingleObject {
        items: Vec<Value>,
    },
    BinaryArray {
        array_type: u8,
        lengths: Vec<i32>,
        lower_bounds: Vec<i32>,
        bt: u8,
        additional: Additional,
        items: Vec<Value>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub arena: Vec<Node>,
    pub libraries: Vec<(i32, String)>,
    /// root arena index
    pub root: usize,
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    fn u8(&mut self) -> Result<u8> {
        if self.remaining() < 1 {
            return err("unexpected end of stream (u8)");
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }
    fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    fn u16(&mut self) -> Result<u16> {
        if self.remaining() < 2 {
            return err("unexpected end of stream (u16)");
        }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }
    fn i32(&mut self) -> Result<i32> {
        if self.remaining() < 4 {
            return err("unexpected end of stream (i32)");
        }
        let v = i32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(self.i32()? as u32)
    }
    fn i64(&mut self) -> Result<i64> {
        if self.remaining() < 8 {
            return err("unexpected end of stream (i64)");
        }
        let v = i64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }
    fn u64(&mut self) -> Result<u64> {
        if self.remaining() < 8 {
            return err("unexpected end of stream (u64)");
        }
        let v = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }
    fn f32(&mut self) -> Result<f32> {
        if self.remaining() < 4 {
            return err("unexpected end of stream (f32)");
        }
        let v = f32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }
    fn f64(&mut self) -> Result<f64> {
        if self.remaining() < 8 {
            return err("unexpected end of stream (f64)");
        }
        let v = f64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }
    fn lps(&mut self) -> Result<String> {
        let mut len: u64 = 0;
        let mut shift = 0u32;
        loop {
            let b = self.u8()?;
            len |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 35 {
                return err("string length encoding too large");
            }
        }
        let len = len as usize;
        if self.remaining() < len {
            return err("unexpected end of stream (string bytes)");
        }
        let s = String::from_utf8(self.data[self.pos..self.pos + len].to_vec())
            .map_err(|_| NrbfError("invalid UTF-8 in length-prefixed string".into()))?;
        self.pos += len;
        Ok(s)
    }
    fn char_value(&mut self) -> Result<String> {
        // BinaryFormatter encodes chars as UTF-8 byte sequences.
        let b = self.u8()?;
        let extra = if b < 0x80 {
            0
        } else if b < 0xE0 {
            1
        } else if b < 0xF0 {
            2
        } else {
            3
        };
        let mut buf = vec![b; 1 + extra];
        for k in 1..=extra {
            buf[k] = self.u8()?;
        }
        String::from_utf8(buf).map_err(|_| NrbfError("invalid UTF-8 in Char primitive".into()))
    }
    fn read_primitive(&mut self, ptype: u8) -> Result<Prim> {
        Ok(match ptype {
            1 => Prim::Boolean(self.u8()? != 0),
            2 => Prim::Byte(self.u8()?),
            3 => Prim::Char(self.char_value()?),
            5 => Prim::Decimal(self.lps()?),
            6 => Prim::Double(self.f64()?),
            7 => Prim::Int16(self.i16()?),
            8 => Prim::Int32(self.i32()?),
            9 => Prim::Int64(self.i64()?),
            10 => Prim::SByte(self.i8()?),
            11 => Prim::Single(self.f32()?),
            12 => Prim::TimeSpan(self.i64()?),
            13 => Prim::DateTime(self.u64()?),
            14 => Prim::UInt16(self.u16()?),
            15 => Prim::UInt32(self.u32()?),
            16 => Prim::UInt64(self.u64()?),
            17 => Prim::Null,
            other => return err(format!("unsupported primitive type {other}")),
        })
    }
}

struct ParseCtx<'a> {
    r: Reader<'a>,
    g: Graph,
    /// objectId -> arena index
    by_id: HashMap<u32, usize>,
    /// objectId of the first definition of a class -> arena index
    class_defs: HashMap<u32, usize>,
}

impl<'a> ParseCtx<'a> {
    fn push_node(&mut self, obj_id: u32, node: Node) -> usize {
        let idx = self.g.arena.len();
        self.g.arena.push(node);
        self.by_id.insert(obj_id, idx);
        idx
    }

    fn read_class_info(&mut self) -> Result<(u32, String, Vec<String>)> {
        let obj_id = self.r.i32()? as u32;
        let name = self.r.lps()?;
        let count = self.r.i32()?;
        if count < 0 || count > 100_000 {
            return err(format!("unreasonable member count {count}"));
        }
        let mut member_names = Vec::with_capacity(count as usize);
        for _ in 0..count {
            member_names.push(self.r.lps()?);
        }
        Ok((obj_id, name, member_names))
    }

    fn read_member_type_info(&mut self, count: usize) -> Result<(Vec<u8>, Vec<Additional>)> {
        let mut binary_types = Vec::with_capacity(count);
        for _ in 0..count {
            binary_types.push(self.r.u8()?);
        }
        let mut additional = Vec::with_capacity(count);
        for bt in &binary_types {
            additional.push(match bt {
                0 | 7 => Additional::Prim(self.r.u8()?),
                3 => Additional::SysClass(self.r.lps()?),
                4 => Additional::Class(self.r.lps()?, self.r.i32()?),
                _ => Additional::None,
            });
        }
        Ok((binary_types, additional))
    }

    fn read_value(&mut self, binary_type: u8, additional: &Additional) -> Result<Value> {
        if binary_type == 0 {
            // Primitive member: value is inline, raw bytes, no record tag.
            if let Additional::Prim(ptype) = additional {
                return Ok(Value::Prim(self.r.read_primitive(*ptype)?));
            }
            return err("primitive member without primitive type");
        }
        self.read_inline_value()
    }

    fn read_inline_value(&mut self) -> Result<Value> {
        let pos = self.r.pos;
        let rec = self.r.u8()?;
        match rec {
            0x06 => Ok(Value::Ref(self.read_binary_object_string()? as usize)),
            0x09 => {
                // NB: MS-NRBF allows member references to appear *before* the
                // referenced record in the stream, so we cannot resolve the id
                // to an arena index yet. We store the raw object id here and
                // rewrite it in `fixup_references` once the whole stream is read.
                Ok(Value::Ref(self.r.i32()? as u32 as usize))
            }
            0x0A => Ok(Value::Null),
            0x01 => Ok(Value::Ref(self.read_class_with_id()? as usize)),
            0x02 => Ok(Value::Ref(self.read_system_class_with_members()? as usize)),
            0x03 => Ok(Value::Ref(self.read_class_with_members()? as usize)),
            0x04 => Ok(Value::Ref(self.read_system_class_with_members_and_types()? as usize)),
            0x05 => Ok(Value::Ref(self.read_class_with_members_and_types()? as usize)),
            0x0F => Ok(Value::Ref(self.read_array_single_primitive()? as usize)),
            0x10 => Ok(Value::Ref(self.read_array_single_object()? as usize)),
            0x11 => Ok(Value::Ref(self.read_array_single_string()? as usize)),
            0x07 => Ok(Value::Ref(self.read_binary_array()? as usize)),
            0x08 => {
                let ptype = self.r.u8()?;
                Ok(Value::Prim(self.r.read_primitive(ptype)?))
            }
            0x0C => {
                self.read_binary_library()?;
                self.read_inline_value()
            }
            0x0D | 0x0E => err(format!(
                "run-length null record 0x{rec:02x} at offset {pos} is only valid inside arrays"
            )),
            other => err(format!("unexpected record type 0x{other:02x} at offset {pos}")),
        }
    }

    fn read_binary_library(&mut self) -> Result<()> {
        let id = self.r.i32()?;
        let name = self.r.lps()?;
        // keep first occurrence; libraries may repeat inline
        if !self.g.libraries.iter().any(|(i, _)| *i == id) {
            self.g.libraries.push((id, name));
        }
        Ok(())
    }

    fn read_binary_object_string(&mut self) -> Result<u32> {
        let obj_id = self.r.i32()? as u32;
        let s = self.r.lps()?;
        self.push_node(obj_id, Node::Str(s));
        Ok(obj_id)
    }

    fn read_members(
        &mut self,
        binary_types: &[u8],
        additional: &[Additional],
    ) -> Result<Vec<Value>> {
        let mut members = Vec::with_capacity(binary_types.len());
        for (bt, ai) in binary_types.iter().zip(additional) {
            members.push(self.read_value(*bt, ai)?);
        }
        Ok(members)
    }

    fn finish_class(
        &mut self,
        obj_id: u32,
        system: bool,
        name: String,
        library_id: i32,
        member_names: Vec<String>,
        binary_types: Vec<u8>,
        additional: Vec<Additional>,
        members: Vec<Value>,
        is_definition: bool,
    ) -> Result<u32> {
        let idx = self.push_node(
            obj_id,
            Node::Class {
                system,
                name,
                library_id,
                member_names,
                binary_types,
                additional,
                members,
            },
        );
        if is_definition {
            self.class_defs.insert(obj_id, idx);
        }
        Ok(obj_id)
    }

    fn read_class_with_members_and_types(&mut self) -> Result<u32> {
        let (obj_id, name, member_names) = self.read_class_info()?;
        let (binary_types, additional) = self.read_member_type_info(member_names.len())?;
        let library_id = self.r.i32()?;
        let members = self.read_members(&binary_types, &additional)?;
        self.finish_class(
            obj_id, false, name, library_id, member_names, binary_types, additional, members, true,
        )
    }

    fn read_system_class_with_members_and_types(&mut self) -> Result<u32> {
        self.read_system_class_with_members_and_types_inner()
    }

    fn read_system_class_with_members_and_types_inner(&mut self) -> Result<u32> {
        let (obj_id, name, member_names) = self.read_class_info()?;
        let (binary_types, additional) = self.read_member_type_info(member_names.len())?;
        let members = self.read_members(&binary_types, &additional)?;
        self.finish_class(
            obj_id, true, name, 0, member_names, binary_types, additional, members, true,
        )
    }

    fn read_system_class_with_members(&mut self) -> Result<u32> {
        let (obj_id, name, member_names) = self.read_class_info()?;
        // SystemClassWithMembers carries no type info; members are all objects.
        let binary_types = vec![2u8; member_names.len()];
        let additional = vec![Additional::None; member_names.len()];
        let members = self.read_members(&binary_types, &additional)?;
        self.finish_class(
            obj_id, true, name, 0, member_names, binary_types, additional, members, true,
        )
    }

    fn read_class_with_members(&mut self) -> Result<u32> {
        // ClassWithMembers: no type info, no library id; members treated as objects.
        let (obj_id, name, member_names) = self.read_class_info()?;
        let binary_types = vec![2u8; member_names.len()];
        let additional = vec![Additional::None; member_names.len()];
        let members = self.read_members(&binary_types, &additional)?;
        self.finish_class(
            obj_id, false, name, 0, member_names, binary_types, additional, members, true,
        )
    }

    fn read_class_with_id(&mut self) -> Result<u32> {
        let obj_id = self.r.i32()? as u32;
        let metadata_id = self.r.i32()? as u32;
        let idx = match self.class_defs.get(&metadata_id).or_else(|| self.by_id.get(&metadata_id)) {
            Some(idx) => *idx,
            None => return err(format!("reference to unknown class metadata id {metadata_id}")),
        };
        let (system, name, library_id, member_names, binary_types, additional) = match &self.g.arena[idx] {
            Node::Class { system, name, library_id, member_names, binary_types, additional, .. } => (
                *system,
                name.clone(),
                *library_id,
                member_names.clone(),
                binary_types.clone(),
                additional.clone(),
            ),
            _ => return err("class metadata id does not point to a class definition"),
        };
        let members = self.read_members(&binary_types, &additional)?;
        self.finish_class(
            obj_id, system, name, library_id, member_names, binary_types, additional, members, false,
        )
    }

    fn read_array_elements(&mut self, length: usize) -> Result<Vec<Value>> {
        let mut items = Vec::with_capacity(length);
        let mut i = 0usize;
        while i < length {
            let pos = self.r.pos;
            let rec = self.r.u8()?;
            match rec {
                0x0A => {
                    items.push(Value::Null);
                    i += 1;
                }
                0x0D => {
                    let n = self.r.u8()? as usize;
                    items.extend(std::iter::repeat(Value::Null).take(n));
                    i += n;
                }
                0x0E => {
                    let n = self.r.i32()? as usize;
                    items.extend(std::iter::repeat(Value::Null).take(n));
                    i += n;
                }
                _ => {
                    self.r.pos = pos;
                    items.push(self.read_inline_value()?);
                    i += 1;
                }
            }
        }
        Ok(items)
    }

    fn read_array_single_primitive(&mut self) -> Result<u32> {
        let obj_id = self.r.i32()? as u32;
        let length = self.r.i32()?;
        if length < 0 {
            return err("negative array length");
        }
        let ptype = self.r.u8()?;
        let mut items = Vec::with_capacity(length as usize);
        for _ in 0..length {
            items.push(self.r.read_primitive(ptype)?);
        }
        self.push_node(obj_id, Node::ArraySinglePrimitive { ptype, items });
        Ok(obj_id)
    }

    fn read_array_single_string(&mut self) -> Result<u32> {
        let obj_id = self.r.i32()? as u32;
        let length = self.r.i32()?;
        if length < 0 {
            return err("negative array length");
        }
        let items = self.read_array_elements(length as usize)?;
        self.push_node(obj_id, Node::ArraySingleString { items });
        Ok(obj_id)
    }

    fn read_array_single_object(&mut self) -> Result<u32> {
        let obj_id = self.r.i32()? as u32;
        let length = self.r.i32()?;
        if length < 0 {
            return err("negative array length");
        }
        let items = self.read_array_elements(length as usize)?;
        self.push_node(obj_id, Node::ArraySingleObject { items });
        Ok(obj_id)
    }

    fn read_binary_array(&mut self) -> Result<u32> {
        let obj_id = self.r.i32()? as u32;
        let array_type = self.r.u8()?;
        let rank = self.r.i32()?;
        if rank <= 0 || rank > 8 {
            return err(format!("unreasonable array rank {rank}"));
        }
        let mut lengths = Vec::with_capacity(rank as usize);
        for _ in 0..rank {
            lengths.push(self.r.i32()?);
        }
        let mut lower_bounds = Vec::with_capacity(rank as usize);
        if matches!(array_type, 3 | 4 | 5) {
            for _ in 0..rank {
                lower_bounds.push(self.r.i32()?);
            }
        }
        let bt = self.r.u8()?;
        let additional = match bt {
            0 | 7 => Additional::Prim(self.r.u8()?),
            3 => Additional::SysClass(self.r.lps()?),
            4 => Additional::Class(self.r.lps()?, self.r.i32()?),
            _ => Additional::None,
        };
        let mut total: i64 = 1;
        for l in &lengths {
            if *l < 0 {
                return err("negative array dimension");
            }
            total *= *l as i64;
        }
        if total > 100_000_000 {
            return err("array too large");
        }
        let items = if bt == 0 {
            if let Additional::Prim(ptype) = additional {
                let mut out = Vec::with_capacity(total as usize);
                for _ in 0..total {
                    out.push(Value::Prim(self.r.read_primitive(ptype)?));
                }
                out
            } else {
                return err("primitive binary array without primitive type");
            }
        } else {
            self.read_array_elements(total as usize)?
        };
        self.push_node(
            obj_id,
            Node::BinaryArray { array_type, lengths, lower_bounds, bt, additional, items },
        );
        Ok(obj_id)
    }
}

/// Rewrite raw object ids stored in member references into arena indices.
fn fixup_references(g: &mut Graph, by_id: &HashMap<u32, usize>) -> Result<()> {
    for idx in 0..g.arena.len() {
        if let Err(e) = fixup_node(g, idx, by_id) {
            return Err(NrbfError(format!("{e} (node {idx}: {})", debug_describe_node(g, idx))));
        }
    }
    Ok(())
}

fn fixup_node(g: &mut Graph, idx: usize, by_id: &HashMap<u32, usize>) -> Result<()> {
    match &mut g.arena[idx] {
        Node::Str(_) | Node::ArraySinglePrimitive { .. } => Ok(()),
        Node::Class { members, .. } => {
            for (mi, v) in members.iter_mut().enumerate() {
                fixup_value(v, by_id).map_err(|e| NrbfError(format!("{e} (member {mi})")))?;
            }
            Ok(())
        }
        Node::ArraySingleString { items } | Node::ArraySingleObject { items } | Node::BinaryArray { items, .. } => {
            for v in items.iter_mut() {
                fixup_value(v, by_id)?;
            }
            Ok(())
        }
    }
}

fn fixup_value(v: &mut Value, by_id: &HashMap<u32, usize>) -> Result<()> {
    if let Value::Ref(raw) = v {
        let old_id = *raw as u32;
        match by_id.get(&old_id) {
            Some(arena) => *v = Value::Ref(*arena),
            None => return err(format!("unresolved object reference: id {old_id}")),
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn debug_describe_node(g: &Graph, idx: usize) -> String {
    match &g.arena[idx] {
        Node::Str(s) => format!("Str({s:?})"),
        Node::Class { name, .. } => format!("Class({name})"),
        Node::ArraySinglePrimitive { ptype, items } => format!("ArraySinglePrimitive(ptype={ptype}, len={})", items.len()),
        Node::ArraySingleString { items } => format!("ArraySingleString(len={})", items.len()),
        Node::ArraySingleObject { items } => format!("ArraySingleObject(len={})", items.len()),
        Node::BinaryArray { bt, lengths, .. } => format!("BinaryArray(bt={bt}, lengths={lengths:?})"),
    }
}

pub fn parse(data: &[u8]) -> Result<Graph> {
    let mut ctx = ParseCtx {
        r: Reader::new(data),
        g: Graph::default(),
        by_id: HashMap::new(),
        class_defs: HashMap::new(),
    };
    // Serialization header
    if ctx.r.u8()? != 0x00 {
        return err("stream does not start with a serialization header record");
    }
    let root_id = ctx.r.i32()? as u32;
    let _header_id = ctx.r.i32()?;
    let _major = ctx.r.i32()?;
    let _minor = ctx.r.i32()?;

    loop {
        if ctx.r.remaining() == 0 {
            return err("stream ended without MessageEnd record");
        }
        let pos = ctx.r.pos;
        let rec = ctx.r.u8()?;
        match rec {
            0x0B => break, // MessageEnd
            0x0C => ctx.read_binary_library()?,
            0x01 => {
                ctx.read_class_with_id()?;
            }
            0x02 => {
                ctx.read_system_class_with_members()?;
            }
            0x03 => {
                ctx.read_class_with_members()?;
            }
            0x04 => {
                ctx.read_system_class_with_members_and_types_inner()?;
            }
            0x05 => {
                ctx.read_class_with_members_and_types()?;
            }
            0x06 => {
                ctx.r.pos = pos;
                ctx.read_inline_value()?;
            }
            0x0F | 0x10 | 0x11 | 0x07 => {
                ctx.r.pos = pos;
                ctx.read_inline_value()?;
            }
            other => return err(format!("unexpected top-level record 0x{other:02x} at offset {pos}")),
        }
    }

    fixup_references(&mut ctx.g, &ctx.by_id)?;

    let root = match ctx.by_id.get(&root_id) {
        Some(idx) => *idx,
        None => return err(format!("root object id {root_id} not found in stream")),
    };
    ctx.g.root = root;
    Ok(ctx.g)
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

pub struct Writer {
    out: Vec<u8>,
    /// arena index -> assigned new object id
    new_ids: HashMap<usize, u32>,
    emitted: HashMap<usize, ()>,
    class_meta: HashMap<String, u32>,
    next_id: u32,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            new_ids: HashMap::new(),
            emitted: HashMap::new(),
            class_meta: HashMap::new(),
            next_id: 1,
        }
    }

    fn w_u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn w_i8(&mut self, v: i8) {
        self.out.push(v as u8);
    }
    fn w_u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_i16(&mut self, v: i16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_i64(&mut self, v: i64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_f32(&mut self, v: f32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_f64(&mut self, v: f64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn w_lps(&mut self, s: &str) {
        self.out.extend_from_slice(&encode_lps(s));
    }
    fn w_char(&mut self, s: &str) {
        self.out.extend_from_slice(s.as_bytes());
    }

    fn assign_id(&mut self, idx: usize) -> u32 {
        if let Some(id) = self.new_ids.get(&idx) {
            return *id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.new_ids.insert(idx, id);
        id
    }

    fn write_primitive(&mut self, p: &Prim) {
        match p {
            Prim::Boolean(v) => self.w_u8(*v as u8),
            Prim::Byte(v) => self.w_u8(*v),
            Prim::SByte(v) => self.w_i8(*v),
            Prim::Char(v) => self.w_char(v),
            Prim::Decimal(v) => self.w_lps(v),
            Prim::Double(v) => self.w_f64(*v),
            Prim::Int16(v) => self.w_i16(*v),
            Prim::Int32(v) => self.w_i32(*v),
            Prim::Int64(v) => self.w_i64(*v),
            Prim::Single(v) => self.w_f32(*v),
            Prim::TimeSpan(v) => self.w_i64(*v),
            Prim::DateTime(v) => self.w_u64(*v),
            Prim::UInt16(v) => self.w_u16(*v),
            Prim::UInt32(v) => self.w_i32(*v as i32),
            Prim::UInt64(v) => self.w_u64(*v),
            Prim::Null => {}
        }
    }

    fn write_additional(&mut self, bt: u8, additional: &Additional) -> Result<()> {
        self.out.extend_from_slice(&additional_for(bt, additional, bt == 7)?);
        Ok(())
    }

    fn write_member_type_infos(&mut self, binary_types: &[u8], additional: &[Additional]) -> Result<()> {
        for bt in binary_types {
            self.w_u8(*bt);
        }
        for (bt, ai) in binary_types.iter().zip(additional) {
            self.write_additional(*bt, ai)?;
        }
        Ok(())
    }

    fn class_signature(system: bool, name: &str, member_names: &[String], binary_types: &[u8], additional: &[Additional]) -> String {
        let mut s = String::new();
        s.push_str(if system { "sys:" } else { "cls:" });
        s.push_str(name);
        s.push('|');
        for n in member_names {
            s.push_str(n);
            s.push(',');
        }
        s.push('|');
        for (bt, ai) in binary_types.iter().zip(additional) {
            s.push_str(&bt.to_string());
            match ai {
                Additional::None => s.push_str("-;"),
                Additional::Prim(p) => {
                    s.push_str(&p.to_string());
                    s.push(';');
                }
                Additional::SysClass(n) => {
                    s.push_str(n);
                    s.push(';');
                }
                Additional::Class(n, l) => {
                    s.push_str(n);
                    s.push('@');
                    s.push_str(&l.to_string());
                    s.push(';');
                }
            }
        }
        s
    }

    fn emit_class_record(&mut self, system: bool, name: &str, new_id: u32) {
        if system {
            self.w_u8(0x04); // SystemClassWithMembersAndTypes
        } else {
            self.w_u8(0x05); // ClassWithMembersAndTypes
        }
        self.w_i32(new_id as i32);
        let _ = name;
    }

    fn class_metadata_key(system: bool, name: &str, member_names: &[String], binary_types: &[u8], additional: &[Additional]) -> String {
        Self::class_signature(system, name, member_names, binary_types, additional)
    }

    fn emit_node(&mut self, g: &Graph, idx: usize) -> Result<()> {
        if self.emitted.contains_key(&idx) {
            return err("internal error: node emitted twice");
        }
        self.emitted.insert(idx, ());
        let is_single_string = matches!(&g.arena[idx], Node::ArraySingleString { .. });
        let node = g.arena[idx].clone();
        match node {
            Node::Str(s) => {
                let id = self.assign_id(idx);
                self.w_u8(0x06);
                self.w_i32(id as i32);
                self.w_lps(&s);
            }
            Node::Class { system, name, library_id, member_names, binary_types, additional, members } => {
                let key = Self::class_metadata_key(system, &name, &member_names, &binary_types, &additional);
                let known_meta = self.class_meta.get(&key).copied();
                if let Some(meta_id) = known_meta {
                    let id = self.assign_id(idx);
                    self.w_u8(0x01); // ClassWithId
                    self.w_i32(id as i32);
                    self.w_i32(meta_id as i32);
                } else {
                    let id = self.assign_id(idx);
                    self.emit_class_record(system, &name, id);
                    self.w_lps(&name);
                    self.w_i32(member_names.len() as i32);
                    for n in &member_names {
                        self.w_lps(n);
                    }
                    self.write_member_type_infos(&binary_types, &additional)?;
                    if !system {
                        self.w_i32(library_id);
                    }
                    self.class_meta.insert(key, id);
                }
                // members
                for (i, bt) in binary_types.iter().enumerate() {
                    let ai = &additional[i];
                    let v = &members[i];
                    if *bt == 0 {
                        match v {
                            Value::Prim(p) => self.write_primitive(p),
                            Value::Null => {}
                            _ => return err(format!("primitive member '{m}' holds a non-primitive value", m = member_names[i])),
                        }
                    } else {
                        match v {
                            Value::Null => self.w_u8(0x0A),
                            Value::Prim(_) => return err(format!("member '{m}' typed as object holds a primitive", m = member_names[i])),
                            Value::Ref(j) => self.emit_value_ref(g, *j)?,
                        }
                    }
                    let _ = ai;
                }
            }
            Node::ArraySinglePrimitive { ptype, items } => {
                let id = self.assign_id(idx);
                self.w_u8(0x0F);
                self.w_i32(id as i32);
                self.w_i32(items.len() as i32);
                self.w_u8(ptype);
                for p in &items {
                    self.write_primitive(p);
                }
            }
            Node::ArraySingleString { items } | Node::ArraySingleObject { items } => {
                let id = self.assign_id(idx);
                let rec = if is_single_string { 0x11 } else { 0x10 };
                self.w_u8(rec);
                self.w_i32(id as i32);
                self.w_i32(items.len() as i32);
                for v in &items {
                    self.emit_value(g, v)?;
                }
            }
            Node::BinaryArray { array_type, lengths, lower_bounds, bt, additional, items } => {
                let id = self.assign_id(idx);
                self.w_u8(0x07);
                self.w_i32(id as i32);
                self.w_u8(array_type);
                self.w_i32(lengths.len() as i32);
                for l in &lengths {
                    self.w_i32(*l);
                }
                for lb in &lower_bounds {
                    self.w_i32(*lb);
                }
                self.w_u8(bt);
                self.write_additional(bt, &additional)?;
                if bt == 0 {
                    for v in &items {
                        match v {
                            Value::Prim(p) => self.write_primitive(p),
                            _ => return err("primitive binary array held non-primitive element"),
                        }
                    }
                } else {
                    for v in &items {
                        self.emit_value(g, v)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_value(&mut self, g: &Graph, v: &Value) -> Result<()> {
        match v {
            Value::Null => self.w_u8(0x0A),
            Value::Prim(p) => self.write_primitive(p),
            Value::Ref(j) => return self.emit_value_ref(g, *j),
        }
        Ok(())
    }

    fn emit_value_ref(&mut self, g: &Graph, j: usize) -> Result<()> {
        let known = self.new_ids.get(&j).copied();
        if let Some(id) = known {
            self.w_u8(0x09);
            self.w_i32(id as i32);
            return Ok(());
        }
        self.emit_node(g, j)
    }

    pub fn write(mut self, g: &Graph) -> Result<Vec<u8>> {
        // Serialization header
        self.w_u8(0x00);
        let root_id_placeholder_offset = self.out.len();
        self.w_i32(0); // patched below
        self.w_i32(-1); // headerId (-1 means no header)
        self.w_i32(1); // major
        self.w_i32(0); // minor

        // Libraries
        for (id, name) in &g.libraries {
            self.w_u8(0x0C);
            self.w_i32(*id);
            self.w_lps(name);
        }

        // Root object inline at top level
        self.assign_id(g.root);
        self.emit_node(g, g.root)?;

        // Message end
        self.w_u8(0x0B);

        // Patch root id
        let root_id = self.new_ids[&g.root] as i32;
        self.out[root_id_placeholder_offset..root_id_placeholder_offset + 4]
            .copy_from_slice(&root_id.to_le_bytes());

        Ok(self.out)
    }
}
