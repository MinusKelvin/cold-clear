use crate::*;

impl From<fumen::Piece> for FallingPiece {
    fn from(v: fumen::Piece) -> FallingPiece {
        FallingPiece {
            kind: PieceState(v.kind.into(), v.rotation.into()),
            x: v.x as i32,
            y: v.y as i32,
            tspin: TspinStatus::None,
        }
    }
}

impl From<fumen::PieceType> for Piece {
    fn from(v: fumen::PieceType) -> Piece {
        match v {
            fumen::PieceType::I => Piece::I,
            fumen::PieceType::O => Piece::O,
            fumen::PieceType::T => Piece::T,
            fumen::PieceType::L => Piece::L,
            fumen::PieceType::J => Piece::J,
            fumen::PieceType::S => Piece::S,
            fumen::PieceType::Z => Piece::Z,
        }
    }
}

impl From<fumen::RotationState> for RotationState {
    fn from(v: fumen::RotationState) -> RotationState {
        match v {
            fumen::RotationState::North => RotationState::North,
            fumen::RotationState::East => RotationState::East,
            fumen::RotationState::West => RotationState::West,
            fumen::RotationState::South => RotationState::South,
        }
    }
}

impl From<FallingPiece> for fumen::Piece {
    fn from(v: FallingPiece) -> fumen::Piece {
        fumen::Piece {
            kind: v.kind.0.into(),
            rotation: v.kind.1.into(),
            x: v.x as u32,
            y: v.y as u32,
        }
    }
}

impl From<Piece> for fumen::PieceType {
    fn from(v: Piece) -> fumen::PieceType {
        match v {
            Piece::I => fumen::PieceType::I,
            Piece::O => fumen::PieceType::O,
            Piece::T => fumen::PieceType::T,
            Piece::L => fumen::PieceType::L,
            Piece::J => fumen::PieceType::J,
            Piece::S => fumen::PieceType::S,
            Piece::Z => fumen::PieceType::Z,
        }
    }
}

impl From<RotationState> for fumen::RotationState {
    fn from(v: RotationState) -> fumen::RotationState {
        match v {
            RotationState::North => fumen::RotationState::North,
            RotationState::East => fumen::RotationState::East,
            RotationState::West => fumen::RotationState::West,
            RotationState::South => fumen::RotationState::South,
        }
    }
}
