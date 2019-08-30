use crate::*;
use std::process::*;
use std::io::{ BufRead, Write, BufReader };
use std::collections::VecDeque;

pub(in super) fn glue(recv: Receiver<BotMsg>, send: Sender<BotResult>) {
    let mut misa = Command::new("C:\\Users\\minus\\Desktop\\MisaMinoBot\\tetris_ai\\dist\\Release\\GNU-Linux\\tetris_ai.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let misa_in = misa.stdin.as_mut().unwrap();
    let mut misa_out = BufReader::new(misa.stdout.as_mut().unwrap());

    writeln!(misa_in, "update _ round 1").unwrap();
    writeln!(misa_in, "settings style 1").unwrap();
    writeln!(misa_in, "settings level 5").unwrap();
    println!("{}", {
        let mut l = String::new();
        misa_out.read_line(&mut l);
        misa_out.read_line(&mut l);
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
            }
            Ok(BotMsg::NextMove) => {
                let next_piece = *queue.front().unwrap();
                writeln!(
                    misa_in,
                    "update _ this_piece_type {}",
                    next_piece.to_char()
                ).unwrap();
                let mut next = String::new();
                for p in &queue {
                    next.push(p.to_char());
                    next.push(',');
                }
                writeln!(misa_in, "update _ next_pieces {}", next).unwrap();
                queue.pop_front();

                writeln!(misa_in, "action2 _ _").unwrap();

                let no_hold_moves = crate::moves::find_moves(
                    &board,
                    FallingPiece::spawn(next_piece, &board).unwrap(),
                    crate::moves::MovementMode::ZeroGFinesse
                );

                let mut line = String::new();
                misa_out.read_line(&mut line).unwrap();
                let mut target = [[false; 10]; 20];
                let mut pipes = 2;
                let mut x = 0;
                let mut y = 19;
                let mut uses_spin = false;
                for c in line.chars() {
                    if pipes != 0 {
                        if c == '|' {
                            pipes -= 1;
                        } else if pipes == 1 {
                            if c != '0' {
                                uses_spin = true;
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
                    for y in 0..20 {
                        for x in 0..10 {
                            if b.occupied(x as i32, y as i32) != target[y][x] {
                                continue 'mvloop;
                            }
                        }
                    }
                    board = b;
                    send.send(BotResult::Move {
                        inputs: mv.inputs,
                        expected_location: mv.location,
                        hold: false
                    });
                    continue 'botloop;
                }
                println!("Found no move to place {} in {}", next_piece.to_char(), line);
                // 'mvloop: for mv in hold_moves {
                //     let mut b = board.clone();
                //     b.lock_piece(mv.location);
                //     for y in 0..20 {
                //         for x in 0..10 {
                //             if b.occupied(x as i32, y as i32) != target[y][x] {
                //                 continue 'mvloop;
                //             }
                //         }
                //     }
                //     send.send(BotResult::Move {
                //         inputs: mv.inputs,
                //         expected_location: mv.location,
                //         hold: true
                //     });
                //     continue 'botloop;
                // }
            }
        }
    }

    misa.kill();
}