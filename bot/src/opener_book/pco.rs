use super::{ OpenerBook, build_states, Opener };
use libtetris::{ Piece::*, PieceState, RotationState::*, FallingPiece, TspinStatus };
use enumset::EnumSet;

pub fn init(states: &mut OpenerBook) {
    // JOL left, ZTS right
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 1, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 1, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, South),
                x: 1, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, West),
                x: 9, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 8, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Pco,
        860
    );
    // LOJ left, ZTS right
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, South),
                x: 1, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, North),
                x: 1, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, West),
                x: 9, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 8, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 7, y: 0,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Pco,
        860
    );
    // JOL left, STZ left
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, North),
                x: 1, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 1, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, South),
                x: 1, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, East),
                x: 3, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 5, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 4, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Pco,
        860
    );
    // LOJ left, STZ left
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(J, South),
                x: 1, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 0, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, North),
                x: 1, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(T, East),
                x: 3, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, North),
                x: 5, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, North),
                x: 4, y: 2,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Pco,
        860
    );

    // Grace System (I base)
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(I, North),
                x: 2, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, East),
                x: 4, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, East),
                x: 0, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, South),
                x: 1, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(J, South),
                x: 4, y: 3,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Pco,
        200
    );

    // Grace System (LJ base)
    build_states(
        states,
        &[
            FallingPiece {
                kind: PieceState(I, North),
                x: 2, y: 3,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(S, East),
                x: 0, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(Z, East),
                x: 4, y: 2,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(O, North),
                x: 2, y: 1,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(L, North),
                x: 4, y: 0,
                tspin: TspinStatus::None
            },
            FallingPiece {
                kind: PieceState(J, North),
                x: 1, y: 0,
                tspin: TspinStatus::None
            }
        ],
        [0; 10],
        EnumSet::all(),
        Opener::Pco,
        200
    );

    states.insert(([
        0b1000111111,
        0b1100111111,
        0b1000111111,
        0b0000111111,
        0, 0, 0, 0, 0, 0
    ], EnumSet::all()), (Opener::Pco, 860));
}