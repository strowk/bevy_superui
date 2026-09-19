/// Id of a node in the JS-side shadow DOM. Root is `1`.
pub type JsNodeId = u32;

/// Index into an [`OpBatch`]'s string pool.
pub type StrId = u32;

/// A single recorded DOM mutation, referencing nodes by [`JsNodeId`] and
/// strings by [`StrId`] into the owning [`OpBatch`]'s pool.
#[derive(PartialEq, Debug, Clone)]
pub enum Op {
    CreateElement { id: JsNodeId, tag: StrId },
    CreateText { id: JsNodeId, data: StrId },
    SetAttribute { id: JsNodeId, name: StrId, value: StrId },
    RemoveAttribute { id: JsNodeId, name: StrId },
    SetProperty { id: JsNodeId, name: StrId, value: StrId },
    SetStyle { id: JsNodeId, prop: StrId, value: StrId },
    SetText { id: JsNodeId, data: StrId },
    /// `reference == 0` means append (insert at end of `parent`'s children).
    InsertBefore { parent: JsNodeId, node: JsNodeId, reference: JsNodeId },
    RemoveChild { parent: JsNodeId, node: JsNodeId },
}

impl Op {
    /// Opcode tag used on the wire. Must stay in sync with [`Op::arity`] and
    /// [`Op::from_operands`]'s match arms.
    fn opcode(&self) -> u8 {
        match self {
            Op::CreateElement { .. } => 0,
            Op::CreateText { .. } => 1,
            Op::SetAttribute { .. } => 2,
            Op::RemoveAttribute { .. } => 3,
            Op::SetProperty { .. } => 4,
            Op::SetStyle { .. } => 5,
            Op::SetText { .. } => 6,
            Op::InsertBefore { .. } => 7,
            Op::RemoveChild { .. } => 8,
        }
    }

    /// Operand `u32` count for each opcode, in field-declaration order.
    fn arity(opcode: u8) -> Option<u32> {
        match opcode {
            0 => Some(2), // CreateElement { id, tag }
            1 => Some(2), // CreateText { id, data }
            2 => Some(3), // SetAttribute { id, name, value }
            3 => Some(2), // RemoveAttribute { id, name }
            4 => Some(3), // SetProperty { id, name, value }
            5 => Some(3), // SetStyle { id, prop, value }
            6 => Some(2), // SetText { id, data }
            7 => Some(3), // InsertBefore { parent, node, reference }
            8 => Some(2), // RemoveChild { parent, node }
            _ => None,
        }
    }

    fn write_operands(&self, out: &mut Vec<u8>) {
        let operands: [u32; 3] = match *self {
            Op::CreateElement { id, tag } => [id, tag, 0],
            Op::CreateText { id, data } => [id, data, 0],
            Op::SetAttribute { id, name, value } => [id, name, value],
            Op::RemoveAttribute { id, name } => [id, name, 0],
            Op::SetProperty { id, name, value } => [id, name, value],
            Op::SetStyle { id, prop, value } => [id, prop, value],
            Op::SetText { id, data } => [id, data, 0],
            Op::InsertBefore { parent, node, reference } => [parent, node, reference],
            Op::RemoveChild { parent, node } => [parent, node, 0],
        };
        let n = Self::arity(self.opcode()).unwrap() as usize;
        for &operand in &operands[..n] {
            out.extend_from_slice(&operand.to_le_bytes());
        }
    }

    fn from_operands(opcode: u8, operands: &[u32]) -> Op {
        match opcode {
            0 => Op::CreateElement { id: operands[0], tag: operands[1] },
            1 => Op::CreateText { id: operands[0], data: operands[1] },
            2 => Op::SetAttribute { id: operands[0], name: operands[1], value: operands[2] },
            3 => Op::RemoveAttribute { id: operands[0], name: operands[1] },
            4 => Op::SetProperty { id: operands[0], name: operands[1], value: operands[2] },
            5 => Op::SetStyle { id: operands[0], prop: operands[1], value: operands[2] },
            6 => Op::SetText { id: operands[0], data: operands[1] },
            7 => Op::InsertBefore { parent: operands[0], node: operands[1], reference: operands[2] },
            8 => Op::RemoveChild { parent: operands[0], node: operands[1] },
            _ => unreachable!("opcode range checked by caller"),
        }
    }
}

/// Why [`OpBatch::decode`] rejected a buffer.
#[derive(PartialEq, Debug, Clone)]
pub enum CodecError {
    /// Buffer ended before a declared field (header, operand, or string)
    /// could be read, or a string's declared bytes were not valid UTF-8.
    Truncated,
    /// An op record's opcode byte was outside `0..=8`.
    BadOpcode(u8),
}

/// Smallest possible encoded op record: 1 opcode byte + the smallest arity
/// (2) worth of `u32` operands. Used to cap a capacity reservation against
/// an untrusted header count (see [`OpBatch::decode`]).
const MIN_OP_RECORD_LEN: usize = 1 + 2 * 4;

/// Smallest possible encoded string record: a `u32` length prefix on an
/// empty string. Used the same way as [`MIN_OP_RECORD_LEN`].
const MIN_STRING_RECORD_LEN: usize = 4;

/// One frame's worth of recorded JS shadow-DOM mutations, plus the string
/// pool its [`Op`]s' [`StrId`]s index into.
#[derive(Default, PartialEq, Debug, Clone)]
pub struct OpBatch {
    pub ops: Vec<Op>,
    pub strings: Vec<String>,
}

impl OpBatch {
    /// Interns `s` into the string pool, deduping against existing entries,
    /// and returns its [`StrId`].
    pub fn intern(&mut self, s: &str) -> StrId {
        if let Some(pos) = self.strings.iter().position(|existing| existing == s) {
            return pos as StrId;
        }
        self.strings.push(s.to_string());
        (self.strings.len() - 1) as StrId
    }

    /// Looks up a string previously returned by [`OpBatch::intern`].
    ///
    /// # Panics
    /// Panics if `s` is out of range for this batch's string pool.
    pub fn resolve(&self, s: StrId) -> &str {
        &self.strings[s as usize]
    }

    /// Encodes this batch to the wire format: LE `u32` header
    /// `{op_count, string_count}`, then op records (`u8` opcode + operand
    /// `u32`s, count per [`Op::arity`]), then the string pool (`u32`
    /// byte-len + UTF-8 bytes, per string).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.ops.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.strings.len() as u32).to_le_bytes());
        for op in &self.ops {
            out.push(op.opcode());
            op.write_operands(&mut out);
        }
        for s in &self.strings {
            let bytes = s.as_bytes();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        out
    }

    /// Decodes a buffer produced by [`OpBatch::encode`].
    pub fn decode(bytes: &[u8]) -> Result<OpBatch, CodecError> {
        let mut cursor = Cursor::new(bytes);
        let op_count = cursor.read_u32()?;
        let string_count = cursor.read_u32()?;

        // op_count/string_count come from the buffer itself, so a forged
        // header (e.g. a huge count with no data behind it) must not drive
        // an unbounded `with_capacity` — cap the reservation by what the
        // remaining bytes could actually hold; the per-field bounds checks
        // below still catch a short buffer either way.
        let ops_cap = (op_count as usize).min(cursor.remaining() / MIN_OP_RECORD_LEN);
        let mut ops = Vec::with_capacity(ops_cap);
        for _ in 0..op_count {
            let opcode = cursor.read_u8()?;
            let arity = Op::arity(opcode).ok_or(CodecError::BadOpcode(opcode))?;
            let mut operands = [0u32; 3];
            for slot in operands.iter_mut().take(arity as usize) {
                *slot = cursor.read_u32()?;
            }
            ops.push(Op::from_operands(opcode, &operands[..arity as usize]));
        }

        let strings_cap = (string_count as usize).min(cursor.remaining() / MIN_STRING_RECORD_LEN);
        let mut strings = Vec::with_capacity(strings_cap);
        for _ in 0..string_count {
            let len = cursor.read_u32()? as usize;
            let raw = cursor.read_bytes(len)?;
            let s = String::from_utf8(raw.to_vec()).map_err(|_| CodecError::Truncated)?;
            strings.push(s);
        }

        Ok(OpBatch { ops, strings })
    }
}

/// Minimal byte-slice reader tracking position, for [`OpBatch::decode`].
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Cursor { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let end = self.pos.checked_add(n).ok_or(CodecError::Truncated)?;
        let slice = self.bytes.get(self.pos..end).ok_or(CodecError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, CodecError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_ops_and_strings() {
        let mut b = OpBatch::default();
        let div = b.intern("div");
        let cls = b.intern("class");
        let v = b.intern("row");
        b.ops.push(Op::CreateElement { id: 2, tag: div });
        b.ops.push(Op::SetAttribute { id: 2, name: cls, value: v });
        b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 0 });
        let bytes = b.encode();
        let back = OpBatch::decode(&bytes).unwrap();
        assert_eq!(back.ops, b.ops);
        assert_eq!(back.resolve(div), "div");
    }

    #[test]
    fn empty_batch_roundtrips() {
        let b = OpBatch::default();
        assert_eq!(OpBatch::decode(&b.encode()).unwrap().ops.len(), 0);
    }

    #[test]
    fn decode_rejects_truncated_buffer() {
        assert!(OpBatch::decode(&[0xff, 0x00]).is_err());
    }

    #[test]
    fn decode_rejects_bad_opcode() {
        // header: 1 op, 0 strings; then an out-of-range opcode byte.
        let mut bytes = [1u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
        bytes.push(255);
        assert_eq!(OpBatch::decode(&bytes), Err(CodecError::BadOpcode(255)));
    }

    #[test]
    fn roundtrip_covers_all_op_variants() {
        let mut b = OpBatch::default();
        let a = b.intern("a");
        let c = b.intern("c");
        let v = b.intern("v");
        b.ops.push(Op::CreateElement { id: 2, tag: a });
        b.ops.push(Op::CreateText { id: 3, data: v });
        b.ops.push(Op::SetAttribute { id: 2, name: c, value: v });
        b.ops.push(Op::RemoveAttribute { id: 2, name: c });
        b.ops.push(Op::SetProperty { id: 2, name: c, value: v });
        b.ops.push(Op::SetStyle { id: 2, prop: c, value: v });
        b.ops.push(Op::SetText { id: 3, data: v });
        b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 3 });
        b.ops.push(Op::RemoveChild { parent: 1, node: 3 });
        let back = OpBatch::decode(&b.encode()).unwrap();
        assert_eq!(back.ops, b.ops);
    }

    #[test]
    fn decode_rejects_truncation_after_valid_header() {
        // Header declares 1 op, 0 strings, but no operand bytes follow.
        let mut header_only = [1u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
        header_only.push(0); // opcode for CreateElement, arity 2 — no operands follow
        assert_eq!(OpBatch::decode(&header_only), Err(CodecError::Truncated));

        // Header declares 0 ops, 1 string, whose declared length exceeds
        // the bytes actually present.
        let mut string_len_lies = [0u32.to_le_bytes(), 1u32.to_le_bytes()].concat();
        string_len_lies.extend_from_slice(&100u32.to_le_bytes());
        string_len_lies.extend_from_slice(b"short");
        assert_eq!(OpBatch::decode(&string_len_lies), Err(CodecError::Truncated));
    }

    #[test]
    fn intern_dedups_identical_strings() {
        let mut b = OpBatch::default();
        let a = b.intern("row");
        let c = b.intern("row");
        assert_eq!(a, c);
        assert_eq!(b.strings.len(), 1);
    }

    #[test]
    fn large_batch_roundtrips() {
        let mut b = OpBatch::default();
        let long_value = "x".repeat(70_000);
        let long_id = b.intern(&long_value);
        let tag = b.intern("div");
        for i in 0..5_000u32 {
            let id = i + 2;
            b.ops.push(Op::CreateElement { id, tag });
            b.ops.push(Op::SetProperty { id, name: tag, value: long_id });
            b.ops.push(Op::InsertBefore { parent: 1, node: id, reference: 0 });
        }
        let bytes = b.encode();
        let back = OpBatch::decode(&bytes).unwrap();
        assert_eq!(back.ops, b.ops);
        assert_eq!(back.resolve(long_id), long_value);
    }
}
