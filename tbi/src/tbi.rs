use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum Message {
    Start {
        hold: Option<Piece>,
        queue: Vec<Piece>,
        combo: u32,
        back_to_back: bool,
        #[serde(with = "BigArray")]
        board: [[Option<char>; 10]; 40],
    },
    Stop,
    Suggest,
    Play {
        #[serde(rename = "move")]
        mv: Move,
    },
    NewPiece {
        piece: Piece,
    },
    Quit,

    Ready {
        name: String,
        version: String,
        author: String,
    },
    Suggestion {
        moves: Vec<Move>,
    },

    #[serde(other)]
    Unknown
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    I,
    O,
    T,
    L,
    J,
    S,
    Z,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub location: PieceLocation,
    pub spin: Spin,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub struct PieceLocation {
    #[serde(rename = "type")]
    pub kind: Piece,
    pub orientation: Orientation,
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    North,
    South,
    East,
    West,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Spin {
    None,
    Mini,
    Full,
}

serde_big_array::big_array!( BigArray; 40, );
