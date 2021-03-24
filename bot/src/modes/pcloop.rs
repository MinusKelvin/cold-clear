use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{ AtomicBool, Ordering };
use arrayvec::ArrayVec;
use libtetris::{ Piece, FallingPiece, Board, LockResult, MovementMode };
use crossbeam_channel::{ Sender, unbounded };
use serde::{ Serialize, Deserialize };
use crate::Move;

pub struct PcLooper {
    current_pc: VecDeque<(Move, LockResult)>,
    abort: Arc<AtomicBool>,
    mode: MovementMode,
    next_pc_queue: VecDeque<Piece>,
    next_pc_hold: Option<Piece>,
    hold_enabled: bool,
    solving: bool,
    priority: PcPriority
}

pub struct PcSolver {
    abort: Arc<AtomicBool>,
    queue: ArrayVec<[pcf::Piece; 11]>,
    hold_enabled: bool,
    priority: PcPriority
}

impl PcLooper {
    pub fn new(board: Board, hold_enabled: bool, mode: MovementMode, priority: PcPriority) -> Self {
        PcLooper {
            current_pc: VecDeque::new(),
            abort: Arc::new(AtomicBool::new(false)),
            next_pc_queue: board.next_queue().collect(),
            next_pc_hold: if hold_enabled { board.hold_piece } else { None },
            hold_enabled,
            solving: false,
            mode, priority
        }
    }

    pub fn think(&mut self) -> Option<PcSolver> {
        if self.solving {
            return None
        }

        let mut queue = ArrayVec::new();
        for &piece in self.next_pc_hold.iter().chain(self.next_pc_queue.iter()).take(11) {
            queue.push(piece.into());
        }

        if !self.hold_enabled && queue.len() >= 10 || queue.len() >= 11 {
            self.solving = true;
            Some(PcSolver {
                abort: self.abort.clone(),
                queue,
                hold_enabled: self.hold_enabled,
                priority: self.priority
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
            let mut solution = ArrayVec::<[_; 10]>::new();
            let mut next_pc_hold = self.next_pc_hold;
            let mut next_pc_queue = self.next_pc_queue.clone();
            for &placement in &soln {
                let placements = libtetris::find_moves(
                    &b,
                    libtetris::SpawnRule::Row19Or20.spawn(placement.kind.0, &b).unwrap(),
                    self.mode
                );

                let mut mv = None;
                for p in placements {
                    if p.location.same_location(&placement) {
                        match &mv {
                            None => mv = Some(p),
                            Some(candidate) => if p.inputs.time < candidate.inputs.time {
                                mv = Some(p)
                            }
                        }
                    }
                }
                if let Some(mv) = mv {
                    let mut mv = Move {
                        expected_location: mv.location,
                        inputs: mv.inputs.movements,
                        hold: false
                    };
    
                    let next = next_pc_queue.pop_front().unwrap();
                    if next != placement.kind.0 {
                        if next_pc_hold.is_none() {
                            next_pc_queue.pop_front().unwrap();
                        }
                        next_pc_hold = Some(next);
                        mv.hold = true;
                    }
    
                    solution.push((mv, b.lock_piece(placement)));
                } else {
                    return;
                }
            }

            for v in solution {
                self.current_pc.push_back(v);
            }
            self.next_pc_queue = next_pc_queue;
            self.next_pc_hold = next_pc_hold;
        }
    }

    pub fn suggest_move(&mut self) -> Result<(Move, Info), bool> {
        match self.current_pc.front() {
            Some((mv, _)) => {
                let mut info = Info {
                    depth: self.current_pc.len() as u32 + 1,
                    plan: vec![]
                };
                for (mv, lock) in &self.current_pc {
                    info.plan.push((mv.expected_location, lock.clone()));
                }
                Ok((mv.clone(), info))
            }
            None => {
                self.abort.store(true, Ordering::Relaxed);
                Err(!self.solving)
            }
        }
    }

    pub fn play_move(&mut self, mv: FallingPiece) -> bool {
        if let Some((mov, _)) = self.current_pc.pop_front() {
            if mov.expected_location.same_location(&mv) {
                !self.current_pc.is_empty() || self.solving
            } else {
                false
            }
        } else {
            false
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
                let mut prev_full = 0;
                for &placement in &soln[..soln.len()-1] {
                    if !pcf::placeability::hard_drop_only(b, placement) {
                        score.long_delays += 1;
                    }
                    b = b.combine(placement.board());
                    let mut full = 0;
                    for y in 0..4 {
                        if b.line_filled(y) {
                            full += 1;
                        }
                    }
                    if full != prev_full {
                        score.long_delays += 1;
                    }
                    let lines_cleared = full - prev_full;
                    let tspin = check_tspin(placement, b);
                    match (lines_cleared, tspin) {
                        (1, true) => score.attack += 2,
                        (2, false) => score.attack += 1,
                        (2, true) => score.attack += 4,
                        (3, false) => score.attack += 2,
                        _ => {}
                    }
                    prev_full = full;
                }
                if !pcf::placeability::hard_drop_only(b, *soln.last().unwrap()) {
                    score.last_placement_long = true;
                }
                match *best {
                    None => *best = Some((soln, score)),
                    Some((_, s)) => if self.priority.cmp(score, s) == std::cmp::Ordering::Greater {
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
                    Some((_, s)) => if self.priority.cmp(score, s) == std::cmp::Ordering::Greater {
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
                result.push(piece.into());
                b = b.combine(placement.board());
            }
            result
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
struct PcScore {
    long_delays: u32,
    last_placement_long: bool,
    attack: u32
}

impl PcPriority {
    fn cmp(self, lhs: PcScore, rhs: PcScore) -> std::cmp::Ordering {
        match self {
            PcPriority::Fastest =>
                lhs.long_delays.cmp(&rhs.long_delays).reverse()
                    .then(lhs.last_placement_long.cmp(&rhs.last_placement_long).reverse())
                    .then(lhs.attack.cmp(&rhs.attack)),
            PcPriority::HighestAttack =>
                lhs.attack.cmp(&rhs.attack)
                    .then(lhs.long_delays.cmp(&rhs.long_delays).reverse())
                    .then(lhs.last_placement_long.cmp(&rhs.last_placement_long).reverse())
        }
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

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Info {
    pub depth: u32,
    pub plan: Vec<(FallingPiece, LockResult)>
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum PcPriority {
    Fastest,
    HighestAttack,
}

fn check_tspin(p: pcf::Placement, b: pcf::BitBoard) -> bool {
    let x = p.x as usize;
    // only doing data entry for 4-line PCs since that's all pc loop mode should ever do
    match p.kind {
        // south states
        pcf::PieceState::TSouth00 =>
            b.cell_filled(x, 0) && b.cell_filled(x + 2, 0) && if b.line_filled(2) {
                !b.line_filled(3) && (b.cell_filled(x, 3) || b.cell_filled(x + 2, 3))
            } else {
                b.cell_filled(x, 2) || b.cell_filled(x + 2, 2)
            },
        pcf::PieceState::TSouth01 =>
            b.cell_filled(x, 0) && b.cell_filled(x + 2, 0)
            && !b.line_filled(3) && (b.cell_filled(x, 3) || b.cell_filled(x + 2, 3)),
        pcf::PieceState::TSouth10 =>
            b.cell_filled(x, 1) && b.cell_filled(x + 2, 1)
            && !b.line_filled(3) && (b.cell_filled(x, 3) || b.cell_filled(x + 2, 3)),

        // east states
        pcf::PieceState::TEast000 =>
            b.cell_filled(x + 1, 0) && b.cell_filled(x + 1, 2)
            && (x == 0 || b.cell_filled(x - 1, 0) || b.cell_filled(x - 1, 2)),
        pcf::PieceState::TEast001 | pcf::PieceState::TEast010 =>
            b.cell_filled(x + 1, 0) && b.cell_filled(x + 1, 3)
            && (x == 0 || b.cell_filled(x - 1, 0) || b.cell_filled(x - 1, 3)),
        pcf::PieceState::TEast100 =>
            b.cell_filled(x + 1, 1) && b.cell_filled(x + 1, 3)
            && (x == 0 || b.cell_filled(x - 1, 1) || b.cell_filled(x - 1, 3)),

        // west states
        pcf::PieceState::TWest000 =>
            b.cell_filled(x, 0) && b.cell_filled(x, 2)
            && (x == 8 || b.cell_filled(x + 2, 0) || b.cell_filled(x + 2, 2)),
        pcf::PieceState::TWest001 | pcf::PieceState::TWest010 =>
            b.cell_filled(x, 0) && b.cell_filled(x, 3)
            && (x == 8 || b.cell_filled(x + 2, 0) || b.cell_filled(x + 2, 3)),
        pcf::PieceState::TWest100 =>
            b.cell_filled(x, 1) && b.cell_filled(x, 3)
            && (x == 8 || b.cell_filled(x + 2, 1) || b.cell_filled(x + 2, 3)),

        // otherwise
        _ => false
    }
}
