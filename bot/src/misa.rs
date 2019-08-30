use crate::*;
use std::process::*;
use std::io::{ BufRead, Write, BufReader };
use std::collections::VecDeque;

struct Mirror<'a>(&'a mut ChildStdin);

impl<'a> Write for Mirror<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)?;
        std::io::stdout().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()?;
        std::io::stdout().flush()
    }
}

pub(in super) fn glue(recv: Receiver<BotMsg>, send: Sender<BotResult>) {
    let mut misa = Command::new("./tetris_ai")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let misa_in = misa.stdin.as_mut().unwrap();
    // let mut misa_in = Mirror(misa.stdin.as_mut().unwrap());
    let mut misa_out = BufReader::new(misa.stdout.as_mut().unwrap());

    writeln!(misa_in, "settings style 1").unwrap();
    writeln!(misa_in, "settings level 10").unwrap();
    println!("{}", {
        let mut l = String::new();
        misa_out.read_line(&mut l).unwrap();
        misa_out.read_line(&mut l).unwrap();
        l
    });

    let mut queue = VecDeque::new();
    let mut board = Board::new();

    'botloop: loop {
        match recv.recv() {
            Err(_) => break,
            Ok(BotMsg::NewPiece(piece)) => {
                queue.push_back(piece);
            }
            Ok(BotMsg::Reset(b)) => {
                board = b;
            }
            Ok(BotMsg::NextMove) => {}
            Ok(BotMsg::PrepareNextMove) => {
                // Reset MisaMino
                writeln!(misa_in, "update _ round 1").unwrap();

                let mut boardspec = String::new();
                for y in (0..20).rev() {
                    for x in 0..10 {
                        boardspec.push(if board.occupied(x, y) {
                            '2'
                        } else {
                            '0'
                        });
                    }
                }
                writeln!(misa_in, "update _ field {}", boardspec).unwrap();

                writeln!(misa_in, "update _ combo {}", board.combo).unwrap();

                let next_piece = queue.pop_front().unwrap();
                writeln!(
                    misa_in,
                    "update _ this_piece_type {}",
                    next_piece.to_char()
                ).unwrap();
                let mut next = String::new();
                if let Some(hold) = board.hold_piece() {
                    next.push(hold.to_char());
                }
                for p in &queue {
                    next.push(p.to_char());
                    next.push(',');
                }

                writeln!(misa_in, "update _ next_pieces {}", next).unwrap();

                writeln!(misa_in, "action2 _ _").unwrap();

                let no_hold_moves = crate::moves::find_moves(
                    &board,
                    FallingPiece::spawn(next_piece, &board).unwrap(),
                    crate::moves::MovementMode::ZeroGComplete
                );

                let hold_piece = board.hold_piece().unwrap_or(*queue.front().unwrap());
                let hold_moves = crate::moves::find_moves(
                    &board,
                    FallingPiece::spawn(hold_piece, &board).unwrap(),
                    crate::moves::MovementMode::ZeroGComplete
                );

                let mut line = String::new();
                misa_out.read_line(&mut line).unwrap();
                let mut target = [[false; 10]; 20];
                let mut pipes = 2;
                let mut x = 0;
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
                                println!("I think it's dead");
                                break 'botloop;
                            } else {
                                lines = c as usize - '0' as usize;
                            }
                        }
                    } else {
                        match c {
                            '2' => target[y][x] = true,
                            ',' => x += 1,
                            ';' => {
                                x = 0;
                                y -= 1;
                            }
                            _ => {}
                        }
                    }
                }
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
                    if uses_spin && !mv.inputs.contains(&crate::moves::Input::SonicDrop) {
                        continue
                    }
                    board = b;
                    send.send(BotResult::Move {
                        inputs: mv.inputs,
                        expected_location: mv.location,
                        hold: false
                    });
                    continue 'botloop;
                }
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
                    if uses_spin && !mv.inputs.contains(&crate::moves::Input::SonicDrop) ||
                            !uses_spin && mv.location.tspin != TspinStatus::None {
                        continue
                    }
                    if board.hold_piece().is_none() {
                        queue.pop_front();
                    }
                    board = b;
                    send.send(BotResult::Move {
                        inputs: mv.inputs,
                        expected_location: mv.location,
                        hold: true
                    });
                    continue 'botloop;
                }
                println!("Couldn't find the placement Misa picked:");
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

    misa.kill();
    misa.wait();
}