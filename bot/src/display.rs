use libtetris::{ Board, LockResult, Row };
use crate::moves::Move;
use arrayvec::ArrayString;

type Drawing = [ArrayString<[u8; 22]>; 28];

pub fn draw_move<R: Row>(
    from_board: &Board<R>,
    to_board: &Board<R>,
    mv: &Move,
    evaluation: Option<i32>,
    depth: u32, garbage: u32, pieces: u32,
    lock_result: &LockResult,
    hold: bool
) -> Drawing {
    let mut drawing = [ArrayString::new(); 28];

    // Inputs
    let mut moves = String::new();
    if hold {
        moves.push('H');
    }
    for i in &mv.inputs {
        moves.push(i.to_char());
    }
    if moves.len() > 22 {
        drawing[0].push_str(&moves[..21]);
        drawing[0].push('*');
    } else {
        drawing[0].push_str(&moves);
        for _ in drawing[0].len()..22 {
            drawing[0].push(' ');
        }
    }

    // Playfield
    let cells = mv.location.cells();
    for (i, y) in (0..22).rev().enumerate() {
        drawing[i+1].push(if y == 20 { '+' } else { '|' });
        for x in 0..10 {
            if cells.contains(&(x, y)) {
                if from_board.occupied(x, y) {
                    drawing[i+1].push_str("??");
                } else {
                    drawing[i+1].push_str("<>");
                }
            } else if from_board.occupied(x, y) {
                drawing[i+1].push_str("[]");
            } else {
                drawing[i+1].push_str("  ");
            }
        }
        drawing[i+1].push(if y == 20 { '+' } else { '|' });
    }

    drawing[23].push_str("+--------------------+");
    // drawing[23].push('+');
    // for x in 0..10 {
    //     use std::fmt::Write;
    //     write!(&mut drawing[23], "{:2}", from_board.column_heights[x]).unwrap();
    // }
    // drawing[23].push('+');

    // Queue
    drawing[24].push('(');
    if let Some(hold) = to_board.hold_piece() {
        drawing[24].push(hold.to_char());
    } else {
        drawing[24].push(' ');
    }
    drawing[24].push(')');
    for (i, c) in to_board.next_queue().enumerate() {
        if i == 20 {
            drawing[24].pop();
            drawing[24].push('*');
            break
        }
        drawing[24].push(c.to_char());
    }
    while !drawing[24].is_full() {
        drawing[24].push(' ');
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
        drawing[25].push_str(&format!("{:13.13}", lock_result.placement_kind.name()));
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
        evaluation.to_string()
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
        garbage.to_string()
    } else {
        format!("{} +{}", garbage, lock_result.garbage_sent)
    };
    let piecestr = format!("#{}", pieces+1);
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