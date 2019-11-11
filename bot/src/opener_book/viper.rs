use super::{ OpenerBook, build_states, Opener };
use libtetris::{ Piece::*, PieceState, RotationState::*, FallingPiece, TspinStatus };
use enumset::EnumSet;

pub fn init(states: &mut OpenerBook) {
    // TSD
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(I, West),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, East),
                x: 1, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, West),
                x: 5, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(J, South),
                x: 8, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 8, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    states.insert(([
        0b1000100111,
        0b0110000001,
        0, 0, 0, 0, 0, 0, 0, 0
    ], EnumSet::all()), (Opener::Viper, 2000));

    // PC 1
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 3, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 3, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, South),
                x: 5, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 2
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, East),
                x: 5, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, East),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 3, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, South),
                x: 3, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 2, y: 3,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 3
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 4, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, East),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, North),
                x: 5, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 2, y: 3,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 4
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, East),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, East),
                x: 5, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, West),
                x: 4, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 5
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, South),
                x: 5, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 6
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 3, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, South),
                x: 5, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 7
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, East),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 2, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 3, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, North),
                x: 5, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 8
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 4, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 3, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 4, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );

    // PC 9
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 3, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, West),
                x: 9, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 4, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 5, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(I, North),
                x: 6, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [
            0b1000100111,
            0b0110000001,
            0b0000000000,
            0b0000000000,
            0, 0, 0, 0, 0, 0
        ],
        EnumSet::all(),
        Opener::Viper,
        0
    );
}