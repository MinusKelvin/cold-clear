use std::collections::HashMap;
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
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MemoryBook(HashMap<Position, Row>);

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
}

impl MemoryBook {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(from: impl BufRead) -> Result<Self, bincode::Error> {
        bincode::deserialize_from(zstd::Decoder::new(from)?)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(from: impl BufRead) -> Result<Self, bincode::Error> {
        bincode::deserialize_from(
            ruzstd::StreamingDecoder::new(&mut { from })
                .map_err(|err| bincode::ErrorKind::Custom(err))?,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save<W: Write>(&self, to: W) -> Result<(), bincode::Error> {
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
        }
    }
}

impl From<MemoryBook> for Book {
    fn from(v: MemoryBook) -> Book {
        Book(BookType::Memory(v))
    }
}
