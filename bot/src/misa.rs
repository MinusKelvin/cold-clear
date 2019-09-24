use crate::*;
use std::process::*;
use std::io::{ BufRead, Write, BufReader };

// struct Mirror<'a>(&'a mut ChildStdin);

// impl<'a> Write for Mirror<'a> {
//     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//         self.0.write(buf)?;
//         std::io::stdout().write(buf)
//     }

//     fn flush(&mut self) -> std::io::Result<()> {
//         self.0.flush()?;
//         std::io::stdout().flush()
//     }
// }

fn mirror(piece: Piece) -> Piece {
    match piece {
        Piece::S => Piece::Z,
        Piece::Z => Piece::S,
        Piece::L => Piece::J,
        Piece::J => Piece::L,
        p => p,
    }
}

pub(in super) fn glue(recv: Receiver<BotMsg>, send: Sender<BotResult>, mut board: Board) {
    let mut misa = Command::new("./tetris_ai")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let misa_in = misa.stdin.as_mut().unwrap();
    // let mut misa_in = Mirror(misa.stdin.as_mut().unwrap());
    let mut misa_out = BufReader::new(misa.stdout.as_mut().unwrap());

    writeln!(misa_in, "settings style 1").unwrap();
    let mut l = String::new();
    misa_out.read_line(&mut l).unwrap();
    writeln!(misa_in, "settings level 10").unwrap();
    l.clear();
    misa_out.read_line(&mut l).unwrap();

    'botloop: loop {
        match recv.recv() {
            Err(_) => break,
            Ok(BotMsg::NewPiece(piece)) => {
                board.add_next_piece(piece);
            }
            Ok(BotMsg::Reset {
                field, combo, b2b
            }) => {
                board.set_field(field);
                board.combo = combo;
                board.b2b_bonus = b2b;
            }
            Ok(BotMsg::NextMove) => {}
            Ok(BotMsg::PrepareNextMove) => {
                // Reset MisaMino
                writeln!(misa_in, "update _ round 1").unwrap();

                let mut boardspec = String::new();
                for y in (0..20).rev() {
                    for x in (0..10).rev() {
                        boardspec.push(if board.occupied(x, y) {
                            '2'
                        } else {
                            '0'
                        });
                    }
                }
                writeln!(misa_in, "update _ field {}", boardspec).unwrap();

                writeln!(misa_in, "update _ combo {}", board.combo).unwrap();

                let next_piece = board.advance_queue().unwrap();
                writeln!(
                    misa_in,
                    "update _ this_piece_type {}",
                    mirror(next_piece).to_char()
                ).unwrap();
                let mut next = String::new();
                if let Some(hold) = board.hold_piece() {
                    next.push(mirror(hold).to_char());
                }
                for p in board.next_queue() {
                    next.push(mirror(p).to_char());
                    next.push(',');
                }

                writeln!(misa_in, "update _ next_pieces {}", next).unwrap();

                writeln!(misa_in, "action2 _ _").unwrap();

                let no_hold_moves = FallingPiece::spawn(next_piece, &board)
                    .map(|p| crate::moves::find_moves(
                            &board, p, crate::moves::MovementMode::ZeroGComplete
                        )
                    );

                let hold_piece = board.hold_piece().unwrap_or(board.get_next_piece().unwrap());
                let hold_moves = FallingPiece::spawn(hold_piece, &board)
                    .map(|p| crate::moves::find_moves(
                            &board, p, crate::moves::MovementMode::ZeroGComplete
                        )
                    );

                let mut line = String::new();
                misa_out.read_line(&mut line).unwrap();
                let mut target = [[false; 10]; 20];
                let mut pipes = 2;
                let mut x = 9;
                let mut y = 19;
                let mut uses_spin = false;
                let mut lines = 0;
                for c in line.chars() {
                    if pipes != 0 {
                        if c == '|' {
                            pipes -= 1;
                        } else if pipes == 1 {
                            if c != '0' {
                                uses_spin = true;
                            }
                        } else if pipes == 2 {
                            if c == '-' {
                                break 'botloop;
                            } else {
                                lines = c as usize - '0' as usize;
                            }
                        }
                    } else {
                        match c {
                            '2' => target[y][x] = true,
                            ',' => x -= 1,
                            ';' => {
                                x = 9;
                                y -= 1;
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(no_hold_moves) = no_hold_moves {
                    'mvloop: for mv in no_hold_moves {
                        let mut b = board.clone();
                        b.lock_piece(mv.location);
                        for y in 0..20-lines {
                            for x in 0..10 {
                                if b.occupied(x as i32, y as i32) != target[y][x] {
                                    continue 'mvloop;
                                }
                            }
                        }
                        if mv.location.kind.0 == Piece::T && (
                            (mv.location.tspin == TspinStatus::None) == uses_spin
                        ) {
                            continue
                        }
                        board = b;
                        send.send(BotResult::Move(Move {
                            inputs: mv.inputs.movements,
                            expected_location: mv.location,
                            hold: false
                        })).ok();
                        continue 'botloop;
                    }
                }
                if let Some(hold_moves) = hold_moves {
                    'mvloop: for mv in hold_moves {
                        let mut b = board.clone();
                        b.hold(next_piece);
                        b.lock_piece(mv.location);
                        for y in 0..20-lines {
                            for x in 0..10 {
                                if b.occupied(x as i32, y as i32) != target[y][x] {
                                    continue 'mvloop;
                                }
                            }
                        }
                        if mv.location.kind.0 == Piece::T && (
                            (mv.location.tspin == TspinStatus::None) == uses_spin
                        ) {
                            continue
                        }
                        if board.hold_piece().is_none() {
                            b.advance_queue();
                        }
                        board = b;
                        send.send(BotResult::Move(Move {
                            inputs: mv.inputs.movements,
                            expected_location: mv.location,
                            hold: true
                        })).ok();
                        continue 'botloop;
                    }
                }
                println!("Couldn't find the placement Misa picked (spin: {}):", uses_spin);
                for y in (0..20).rev() {
                    for x in 0..10 {
                        if target[y][x] {
                            print!("[]")
                        } else {
                            print!("  ")
                        }
                    }
                    println!("|");
                }
                println!("Note: {} next, {} in hold", next_piece.to_char(), hold_piece.to_char());
            }
        }
    }

    misa.kill().ok();
    misa.wait().ok();
}