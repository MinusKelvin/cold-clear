use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{ AtomicBool, Ordering };
use arrayvec::ArrayVec;
use libtetris::{ Piece, RotationState, PieceState, TspinStatus, FallingPiece, Board, LockResult };
use crossbeam_channel::{ Sender, unbounded };
use crate::{ Move, Info };

pub struct PcLooper {
    current_pc: VecDeque<(Move, LockResult)>,
    abort: Arc<AtomicBool>,
    next_pc_queue: VecDeque<Piece>,
    next_pc_hold: Option<Piece>,
    hold_enabled: bool,
    solving: bool
}

pub struct PcSolver {
    abort: Arc<AtomicBool>,
    queue: ArrayVec<[pcf::Piece; 11]>,
    hold_enabled: bool
}

impl PcLooper {
    pub fn new(board: Board, hold_enabled: bool) -> Self {
        PcLooper {
            current_pc: VecDeque::new(),
            abort: Arc::new(AtomicBool::new(false)),
            next_pc_queue: board.next_queue().collect(),
            next_pc_hold: if hold_enabled { board.hold_piece } else { None },
            hold_enabled,
            solving: false
        }
    }

    pub fn think(&mut self) -> Option<PcSolver> {
        if self.solving {
            return None
        }

        let mut queue = ArrayVec::new();
        for &piece in self.next_pc_hold.iter().chain(self.next_pc_queue.iter()).take(11) {
            queue.push(match piece {
                Piece::I => pcf::Piece::I,
                Piece::S => pcf::Piece::S,
                Piece::Z => pcf::Piece::Z,
                Piece::O => pcf::Piece::O,
                Piece::T => pcf::Piece::T,
                Piece::L => pcf::Piece::L,
                Piece::J => pcf::Piece::J
            });
        }

        if !self.hold_enabled && queue.len() >= 10 || queue.len() >= 11 {
            self.solving = true;
            Some(PcSolver {
                abort: self.abort.clone(),
                queue,
                hold_enabled: self.hold_enabled
            })
        } else {
            None
        }
    }

    pub fn solution(&mut self, soln: Option<ArrayVec<[FallingPiece; 10]>>) {
        self.solving = false;
        self.abort.store(false, Ordering::Relaxed);
        
        if let Some(soln) = soln {
            let mut b = Board::<u16>::new();
            for &placement in &soln {
                let placements = crate::moves::find_moves(
                    &b,
                    FallingPiece::spawn(placement.kind.0, &b).unwrap(),
                    crate::moves::MovementMode::ZeroG
                );

                let mut target_cells = placement.cells();
                target_cells.sort();
                let mut mv = None;
                for p in placements {
                    let mut cells = p.location.cells();
                    cells.sort();
                    if cells == target_cells {
                        match &mv {
                            None => mv = Some(p),
                            Some(candidate) => if p.inputs.time < candidate.inputs.time {
                                mv = Some(p)
                            }
                        }
                    }
                }
                if mv.is_none() {
                    eprintln!("{:#?} {:#?} {:#?}", b, placement, &soln);
                }
                let mv = mv.unwrap();
                let mut mv = Move {
                    expected_location: mv.location,
                    inputs: mv.inputs.movements,
                    hold: false
                };

                let next = self.next_pc_queue.pop_front().unwrap();
                if next != placement.kind.0 {
                    if self.next_pc_hold.is_none() {
                        self.next_pc_queue.pop_front().unwrap();
                    }
                    self.next_pc_hold = Some(next);
                    mv.hold = true;
                }

                self.current_pc.push_back((mv, b.lock_piece(placement)));
            }
        }
    }

    pub fn next_move(&mut self) -> Result<(Move, Info), bool> {
        match self.current_pc.pop_front() {
            Some((mv, lock)) => {
                let mut info = Info {
                    depth: self.current_pc.len() as u32 + 1,
                    nodes: 0,
                    original_rank: 0,
                    plan: vec![]
                };
                info.plan.push((mv.expected_location, lock));
                for (mv, lock) in &self.current_pc {
                    info.plan.push((mv.expected_location, lock.clone()));
                }
                Ok((mv, info))
            }
            None => {
                self.abort.store(true, Ordering::Relaxed);
                Err(!self.solving)
            }
        }
    }

    pub fn add_next_piece(&mut self, piece: Piece) {
        self.next_pc_queue.push_back(piece);
    }
}

impl Drop for PcLooper {
    fn drop(&mut self) {
        self.abort.store(true, Ordering::Relaxed);
    }
}

impl PcSolver {
    pub fn solve(&self) -> Option<ArrayVec<[FallingPiece; 10]>> {
        let (send, recv) = unbounded();

        let mut best = SendOnDrop::new(None, send);
        pcf::solve_pc_mt(
            &self.queue, pcf::BitBoard(0), self.hold_enabled, false, &self.abort,
            pcf::placeability::simple_srs_spins,
            move |soln| {
                let soln: ArrayVec<[_; 10]> = soln.iter().copied().collect();
                let mut score = PcScore::default();
                let mut b = pcf::BitBoard(0);
                let mut prev_cleared = 0;
                for &placement in &soln[..soln.len()-1] {
                    if !pcf::placeability::hard_drop_only(b, placement) {
                        score.long_delays += 1;
                    }
                    b = b.combine(placement.board());
                    let mut cleared = 0;
                    for y in 0..4 {
                        if b.line_filled(y) {
                            cleared += 1;
                        }
                    }
                    if cleared != prev_cleared {
                        score.long_delays += 1;
                    }
                    if cleared - prev_cleared == 2 {
                        score.attack += 1;
                    }
                    if cleared - prev_cleared == 3 {
                        score.attack += 2;
                    }
                    prev_cleared = cleared;
                }
                if !pcf::placeability::hard_drop_only(b, *soln.last().unwrap()) {
                    score.last_placement_long = true;
                }
                match *best {
                    None => *best = Some((soln, score)),
                    Some((_, s)) => if score > s {
                        *best = Some((soln, score));
                    }
                }
            }
        );

        let mut best = None;
        for candidate in recv {
            if let Some((soln, score)) = candidate {
                match best {
                    None => best = Some((soln, score)),
                    Some((_, s)) => if score > s {
                        best = Some((soln, score))
                    }
                }
            }
        }

        best.map(|(soln, _)| {
            let mut result = ArrayVec::new();
            let mut b = pcf::BitBoard(0);
            for &placement in &soln {
                let piece = placement.srs_piece(b)[0];
                result.push(FallingPiece {
                    kind: PieceState(match piece.piece {
                        pcf::Piece::I => Piece::I,
                        pcf::Piece::J => Piece::J,
                        pcf::Piece::L => Piece::L,
                        pcf::Piece::S => Piece::S,
                        pcf::Piece::Z => Piece::Z,
                        pcf::Piece::T => Piece::T,
                        pcf::Piece::O => Piece::O,
                    }, match piece.rotation {
                        pcf::Rotation::North => RotationState::North,
                        pcf::Rotation::South => RotationState::South,
                        pcf::Rotation::West => RotationState::West,
                        pcf::Rotation::East => RotationState::East,
                    }),
                    x: piece.x,
                    y: piece.y,
                    tspin: TspinStatus::None
                });
                b = b.combine(placement.board());
            }
            result
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, Default)]
struct PcScore {
    long_delays: u32,
    last_placement_long: bool,
    attack: u32
}

impl PartialOrd for PcScore {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.long_delays.cmp(&rhs.long_delays).reverse()
            .then(self.last_placement_long.cmp(&rhs.last_placement_long).reverse())
            .then(self.attack.cmp(&rhs.attack)))
    }
}

#[derive(Clone)]
struct SendOnDrop<T>(std::mem::ManuallyDrop<T>, Sender<T>);

impl<T> SendOnDrop<T> {
    fn new(v: T, sender: Sender<T>) -> Self {
        SendOnDrop(std::mem::ManuallyDrop::new(v), sender)
    }
}

impl<T> std::ops::Deref for SendOnDrop<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for SendOnDrop<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Drop for SendOnDrop<T> {
    fn drop(&mut self) {
        self.1.send(unsafe { std::mem::ManuallyDrop::take(&mut self.0) }).ok();
    }
}