use crate::tetris::{ BoardState, LockResult };
use crate::moves::Move;
use arrayvec::ArrayString;

type Drawing = [ArrayString<[u8; 22]>; 28];

pub fn draw_move(
    board: &BoardState,
    mv: &Move,
    evaluation: Option<i32>,
    depth: u32,
    lock_result: LockResult
) -> Drawing {
    let mut drawing = [ArrayString::new(); 28];

    // Inputs
    let moves = mv.inputs.iter()
        .map(|&i| i.to_char())
        .take(21)
        .chain(std::iter::repeat(
            if mv.inputs.len() < 22 {
                ' '
            } else if mv.inputs.len() == 22 {
                mv.inputs[21].to_char()
            } else {
                '*'
            }
        ))
        .take(22);
    for c in moves {
        drawing[0].push(c);
    }

    // Playfield
    let cells = mv.location.cells();
    for (i, y) in (0..22).rev().enumerate() {
        drawing[i+1].push(if y == 20 { '+' } else { '|' });
        for x in 0..10 {
            if cells.contains(&(x, y)) {
                drawing[i+1].push_str("<>");
            } else if board.occupied(x, y) {
                drawing[i+1].push_str("[]");
            } else {
                drawing[i+1].push_str("  ");
            }
        }
        drawing[i+1].push(if y == 20 { '+' } else { '|' });
    }
    drawing[23].push_str("+--------------------+");

    // Queue
    drawing[24].push('(');
    if let Some(hold) = board.hold_piece {
        drawing[24].push(hold.to_char());
    } else {
        drawing[24].push(' ');
    }
    drawing[24].push(')');
    let pieces = board.next_pieces.iter()
        .skip(1)
        .map(|&i| i.to_char())
        .take(18)
        .chain(std::iter::repeat(
            if board.next_pieces.len() < 20 {
                ' '
            } else if board.next_pieces.len() == 20 {
                board.next_pieces[8].to_char()
            } else {
                '*'
            }
        ))
        .take(19);
    for c in pieces {
        drawing[24].push(c);
    }

    // Lock result
    if lock_result.b2b {
        drawing[25].push_str("B2B ");
    } else {
        drawing[25].push_str("    ");
    }
    if lock_result.perfect_clear {
        drawing[25].push_str("Perfect Clear");
    } else {
        drawing[25].push_str(lock_result.clear_kind.name());
    }
    if let Some(combo) = lock_result.combo {
        let combo_text = format!(" x{}", combo);
        for _ in combo_text.len()..5 {
            drawing[25].push(' ');
        }
        drawing[25].push_str(&combo_text);
    } else {
        drawing[25].push_str("     ");
    }

    // Evaluation and depth
    let evalstr = if let Some(evaluation) = evaluation {
        (evaluation - board.total_garbage as i32 * 100).to_string()
    } else {
        "DEAD".to_owned()
    };
    let depthstr = format!("({})", depth);
    drawing[26].push_str(&evalstr);
    for _ in evalstr.len()..22-depthstr.len() {
        drawing[26].push(' ');
    }
    drawing[26].push_str(&depthstr);

    // Garbage sent, piece count
    let garbstr = if lock_result.garbage_sent == 0 {
        board.total_garbage.to_string()
    } else {
        format!("{} +{}", board.total_garbage, lock_result.garbage_sent)
    };
    let piecestr = format!("#{}", board.piece_count+1);
    drawing[27].push_str(&garbstr);
    for _ in piecestr.len()..22-garbstr.len() {
        drawing[27].push(' ');
    }
    drawing[27].push_str(&piecestr);

    drawing
}

pub fn write_drawings(
    to: &mut impl std::io::Write,
    drawings: &[Drawing]
) -> Result<(), std::io::Error> {
    for chunk in drawings.chunks(3) {
        for row in 0..28 {
            for drawing in chunk {
                write!(to, " {} ", drawing[row])?;
            }
            writeln!(to)?;
        }
        writeln!(to)?;
    }
    Ok(())
}