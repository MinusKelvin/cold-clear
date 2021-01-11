use crate::*;

impl From<pcf::SrsPiece> for FallingPiece {
    fn from(v: pcf::SrsPiece) -> FallingPiece {
        FallingPiece {
            kind: PieceState(v.piece.into(), v.rotation.into()),
            x: v.x,
            y: v.y,
            tspin: TspinStatus::None,
        }
    }
}

impl From<pcf::Piece> for Piece {
    fn from(v: pcf::Piece) -> Piece {
        match v {
            pcf::Piece::I => Piece::I,
            pcf::Piece::O => Piece::O,
            pcf::Piece::T => Piece::T,
            pcf::Piece::L => Piece::L,
            pcf::Piece::J => Piece::J,
            pcf::Piece::S => Piece::S,
            pcf::Piece::Z => Piece::Z,
        }
    }
}

impl From<pcf::Rotation> for RotationState {
    fn from(v: pcf::Rotation) -> RotationState {
        match v {
            pcf::Rotation::North => RotationState::North,
            pcf::Rotation::East => RotationState::East,
            pcf::Rotation::West => RotationState::West,
            pcf::Rotation::South => RotationState::South,
        }
    }
}

impl From<FallingPiece> for pcf::SrsPiece {
    fn from(v: FallingPiece) -> pcf::SrsPiece {
        pcf::SrsPiece {
            piece: v.kind.0.into(),
            rotation: v.kind.1.into(),
            x: v.x,
            y: v.y,
        }
    }
}

impl From<Piece> for pcf::Piece {
    fn from(v: Piece) -> pcf::Piece {
        match v {
            Piece::I => pcf::Piece::I,
            Piece::O => pcf::Piece::O,
            Piece::T => pcf::Piece::T,
            Piece::L => pcf::Piece::L,
            Piece::J => pcf::Piece::J,
            Piece::S => pcf::Piece::S,
            Piece::Z => pcf::Piece::Z,
        }
    }
}

impl From<RotationState> for pcf::Rotation {
    fn from(v: RotationState) -> pcf::Rotation {
        match v {
            RotationState::North => pcf::Rotation::North,
            RotationState::East => pcf::Rotation::East,
            RotationState::West => pcf::Rotation::West,
            RotationState::South => pcf::Rotation::South,
        }
    }
}
