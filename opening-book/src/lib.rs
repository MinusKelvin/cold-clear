use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

use enumset::EnumSet;
use libtetris::{Board, FallingPiece, Piece};
use serde::{Deserialize, Serialize};

const NEXT_PIECES: usize = 4;

#[cfg(feature = "builder")]
mod builder;
#[cfg(feature = "builder")]
pub use builder::*;
#[cfg(feature = "builder")]
pub use data::Position;

mod data;
use crate::data::*;

pub struct Book(BookType);

enum BookType {
    Memory(MemoryBook),
    #[cfg(not(target_arch = "wasm32"))]
    Disk(DiskBook),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MemoryBook(HashMap<Position, Row>);

#[cfg(not(target_arch = "wasm32"))]
pub struct DiskBook {
    index: HashMap<Position, (u64, u64)>,
    file: File,
    dict: zstd::dict::DecoderDictionary<'static>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
struct Row(Box<[(Sequence, Option<CompactPiece>)]>);

impl Row {
    fn lookup(&self, seq: &Sequence) -> Option<FallingPiece> {
        match self.0.binary_search_by_key(seq, |&(s, _)| s) {
            Result::Ok(i) => self.0[i].1.map(Into::into),
            Result::Err(i) => self.0[i - 1].1.map(Into::into),
        }
    }

    fn custom_serialize(&self, mut to: impl Write) -> std::io::Result<()> {
        for &(seq, p) in &*self.0 {
            let [piece_low, piece_high] = p.map(|p| p.0.into()).unwrap_or(0).to_le_bytes();
            to.write(&[
                seq.next.try_as_u8().unwrap(),
                seq.queue[0] as u8,
                seq.queue[1] as u8,
                seq.queue[2] as u8,
                seq.queue[3] as u8,
                piece_low,
                piece_high,
            ])?;
        }
        Ok(())
    }

    fn custom_deserialize(mut from: impl Read) -> std::io::Result<Self> {
        fn bad_data() -> std::io::Error {
            std::io::Error::new(std::io::ErrorKind::Other, "Bad data")
        }
        fn conv_piece(v: u8) -> std::io::Result<Piece> {
            match v {
                0 => Ok(Piece::I),
                1 => Ok(Piece::O),
                2 => Ok(Piece::T),
                3 => Ok(Piece::L),
                4 => Ok(Piece::J),
                5 => Ok(Piece::S),
                6 => Ok(Piece::Z),
                _ => Err(bad_data()),
            }
        }
        let mut result = vec![];
        loop {
            let mut buf = [0; 7];
            match from.read_exact(&mut buf) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(Row(result.into_boxed_slice()))
                }
                Err(e) => return Err(e),
            }
            let seq = Sequence {
                next: EnumSet::try_from_u8(buf[0]).ok_or(bad_data())?,
                queue: [
                    conv_piece(buf[1])?,
                    conv_piece(buf[2])?,
                    conv_piece(buf[3])?,
                    conv_piece(buf[4])?,
                ],
            };
            let mv = u16::from_le_bytes(buf[5..7].try_into().unwrap());
            if mv != 0 && mv & 0b111 == 0 {
                return Err(bad_data());
            }
            let mv = CompactPiece::from_u16(mv);
            result.push((seq, mv));
        }
    }
}

impl MemoryBook {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(from: impl BufRead) -> bincode::Result<Self> {
        bincode::deserialize_from(zstd::Decoder::new(from)?)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(from: impl BufRead) -> bincode::Result<Self> {
        bincode::deserialize_from(
            ruzstd::StreamingDecoder::new(&mut { from })
                .map_err(|err| bincode::ErrorKind::Custom(err))?,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save<W: Write>(&self, to: W) -> bincode::Result<()> {
        let mut to = zstd::Encoder::new(to, 19)?;
        to.multithread(num_cpus::get() as u32)?;
        bincode::serialize_into(&mut to, self)?;
        to.finish()?;
        Ok(())
    }

    pub fn suggest_move(&self, state: &Board) -> Option<FallingPiece> {
        let (pos, seq) = decompose_board(state)?;
        self.0.get(&pos)?.lookup(&seq)
    }

    pub fn merge(&mut self, other: MemoryBook) {
        for (pos, data) in other.0 {
            self.0.entry(pos).or_insert(data);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_as_disk_book(&self, mut to: impl Write) -> bincode::Result<()> {
        to.write_all(&DiskBook::MAGIC_BYTES)?;
        let mut index = HashMap::with_capacity(self.0.len());

        let dict = zstd::dict::EncoderDictionary::new(include_bytes!("dictionary"), 19);

        let mut offset = 4;
        for (&pos, row) in &self.0 {
            if row.0.len() == 1 {
                index.insert(
                    pos,
                    (row.0[0].1.map(|v| v.0.into()).unwrap_or(0) as u64, 0u64),
                );
            } else if row.0.len() == 2 {
                let v1 = row.0[0].1.map(|v| v.0.into()).unwrap_or(0);
                let mut buf = [0; 8];
                Row(vec![row.0[1]].into_boxed_slice()).custom_serialize(buf.as_mut())?;
                index.insert(pos, (u64::from_le_bytes(buf), (v1 as u64) << 32 | 1 << 24));
            } else {
                let mut buf = vec![];
                let mut encoder = zstd::Encoder::with_prepared_dictionary(&mut buf, &dict)?;
                encoder.include_magicbytes(false)?;
                encoder.include_contentsize(false)?;
                encoder.include_checksum(false)?;
                encoder.include_dictid(false)?;
                row.custom_serialize(&mut encoder)?;
                encoder.finish()?;

                to.write_all(&buf)?;
                index.insert(pos, (offset, buf.len() as u64));
                offset += buf.len() as u64;
            }
        }

        let mut buf = vec![];
        let mut w = zstd::stream::Encoder::new(&mut buf, 19)?;
        bincode::serialize_into(&mut w, &index)?;
        w.finish()?;
        to.write_all(buf.as_slice())?;
        to.write_all((buf.len() as u64).to_le_bytes().as_ref())?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl DiskBook {
    const MAGIC_BYTES: [u8; 4] = [0xB7, 0x1E, 0xA0, 0x73];
    const MAGIC: u32 = u32::from_le_bytes(Self::MAGIC_BYTES);

    pub fn load(mut file: File) -> bincode::Result<Self> {
        let mut magic = [0; 4];
        file.read_exact(&mut magic)?;
        if magic != Self::MAGIC_BYTES {
            return Err(serde::de::Error::custom("Invalid CC book file"));
        }

        file.seek(SeekFrom::End(-8))?;
        let mut index_size = [0; 8];
        file.read_exact(&mut index_size)?;
        let index_size = u64::from_le_bytes(index_size);

        file.seek(SeekFrom::End(-8 - index_size as i64))?;
        let mut buf = vec![0; index_size as usize];
        file.read_exact(&mut buf)?;
        let index = zstd::stream::decode_all(buf.as_slice())?;
        let index = bincode::deserialize(&index)?;

        let dict = zstd::dict::DecoderDictionary::new(include_bytes!("dictionary"));

        Ok(DiskBook { file, index, dict })
    }

    pub fn suggest_move(&self, state: &Board) -> Option<FallingPiece> {
        let (pos, seq) = decompose_board(state)?;
        let &(offset, length) = self.index.get(&pos)?;
        if length & (1 << 24) - 1 == 0 {
            if length == 0 {
                CompactPiece::from_u16(offset as u16).map(Into::into)
            } else {
                let row = Row::custom_deserialize(&offset.to_le_bytes()[..7]).ok()?;
                let s = row.0[0].0;
                let v = row.0[0].1;
                if seq < s {
                    CompactPiece::from_u16((length >> 32) as u16).map(Into::into)
                } else {
                    v.map(Into::into)
                }
            }
        } else {
            let mut buf = vec![0; length as usize];
            Self::read_raw(&self.file, offset, &mut buf).ok()?;
            let mut decoder =
                zstd::Decoder::with_prepared_dictionary(buf.as_slice(), &self.dict).ok()?;
            decoder.include_magicbytes(false).ok()?;
            Row::custom_deserialize(decoder).ok()?.lookup(&seq)
        }
    }

    #[cfg(unix)]
    fn read_raw(file: &File, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        use std::os::unix::fs::FileExt;
        file.read_exact_at(buf, offset)
    }

    #[cfg(windows)]
    fn read_raw(file: &File, mut offset: u64, mut buf: &mut [u8]) -> std::io::Result<()> {
        use std::os::windows::fs::FileExt;
        while !buf.is_empty() {
            let read = file.seek_read(buf, offset)?;
            buf = &mut buf[read..];
            offset += read as u64;
        }
        Ok(())
    }
}

impl Book {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: impl AsRef<Path>) -> Result<Self, bincode::Error> {
        let mut file = File::open(path)?;
        let mut magic = [0; 4];
        file.read_exact(&mut magic)?;
        file.seek(SeekFrom::Start(0))?;
        match u32::from_le_bytes(magic) {
            // this is just the zstd header since saved memory books are just zstd'd bincode
            0xFD2FB528 => MemoryBook::load(std::io::BufReader::new(file)).map(Into::into),
            DiskBook::MAGIC => DiskBook::load(file).map(Into::into),
            _ => Err(serde::de::Error::custom("Invalid file")),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(from: impl BufRead) -> Result<Self, bincode::Error> {
        MemoryBook::load(from).map(Into::into)
    }

    pub fn suggest_move(&self, state: &Board) -> Option<FallingPiece> {
        match &self.0 {
            BookType::Memory(b) => b.suggest_move(state),
            #[cfg(not(target_arch = "wasm32"))]
            BookType::Disk(b) => b.suggest_move(state),
        }
    }
}

impl From<MemoryBook> for Book {
    fn from(v: MemoryBook) -> Book {
        Book(BookType::Memory(v))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<DiskBook> for Book {
    fn from(v: DiskBook) -> Book {
        Book(BookType::Disk(v))
    }
}
