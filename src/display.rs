use crate::tetris::BoardState;
use crate::moves::Move;
use arrayvec::ArrayString;

type Drawing = [ArrayString<[u8; 12]>; 22];

pub fn draw_move(board: &BoardState, mv: &Move, evaluation: i32) -> Drawing {
    let mut drawing = [ArrayString::new(); 22];
    let evalstring = evaluation.to_string();
    drawing[21].push('+');
    drawing[21].push_str(&evalstring);
    for _ in evalstring.len()..10 {
        drawing[21].push('-');
    }
    drawing[21].push('+');
    let moves = mv.inputs.iter()
        .map(|&i| i.to_char())
        .take(11)
        .chain(std::iter::repeat(
            if mv.inputs.len() < 12 {
                ' '
            } else if mv.inputs.len() == 12 {
                mv.inputs[11].to_char()
            } else {
                '*'
            }
        ))
        .take(12);
    for c in moves {
        drawing[0].push(c);
    }

    let cells = mv.location.cells();
    for (i, y) in (0..20).into_iter().rev().enumerate() {
        drawing[i+1].push('|');
        for x in 0..10 {
            drawing[i+1].push(if cells.contains(&(x, y)) {
                'O'
            } else if board.occupied(x, y) {
                '#'
            } else {
                ' '
            });
        }
        drawing[i+1].push('|');
    }

    drawing
}

pub fn write_drawings(
    to: &mut impl std::io::Write,
    drawings: &[Drawing]
) -> Result<(), std::io::Error> {
    for chunk in drawings.chunks(6) {
        for row in 0..22 {
            for drawing in chunk {
                write!(to, "{} ", drawing[row])?;
            }
            writeln!(to)?;
        }
        writeln!(to)?;
    }
    Ok(())
}