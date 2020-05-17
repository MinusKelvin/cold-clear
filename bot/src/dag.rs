//! This file implements the DAG structure Cold Clear uses to store think results and facilitate the
//! Monte Carlo Tree Search. The is by far the most complicated file in Cold Clear.
//! 
//! ## Implementation notes
//! 
//! A generation contains all of the nodes that involve placing a specific number of pieces on
//! the board. `DagState.node_generations[0]` is always the generation of the current board state
//! and always belongs to generation `DagState.pieces`. When a move is picked, the generation the
//! root belonged to is deleted.
//! 
//! Generations are associated with pieces in the next queue in a natural way. If the associated
//! piece is not known, the generation is a speculated generation. It follows that there are never
//! known generations following the first speculated generation. When a new piece is added to the
//! next queue, the associated generation is converted from speculated to known. This process does
//! not change the slab keys of 
//! 
//! The piece associated with the generation is the last piece of information required to allow the
//! children to be known instead of speculated. There are three cases:
//! 1. Hold is disabled: The generation piece is the next piece.
//! 2. Hold is empty: The reserve piece is the next piece, the generation piece is the piece after.
//! 3. Hold is full: The reserve piece is the hold piece, the generation piece is the next piece.
//! The same board state with the same reserve piece can be reached but one path may leave hold
//! empty and the other fill the hold slot. `SimplifiedBoard.reserve_is_hold` distinguishes these
//! cases.
//! 
//! Traversing the DAG  requires cloning the `DagState.board` and calling `Board::lock_piece` every
//! time you traverse down if you want information relating to the board anywhere in the DAG. This
//! might slow down `find_and_mark_leaf`, but that function is already very fast.
//! 
//! ## On memory allocation
//! 
//! Hitting the memory allocator appears to be horribly slow on Windows. I'm trying to avoid this
//! by only de/allocating in large chunks, using `bumpalo` for the children lists is the main
//! optimization there. `bumpalo` has an interesting strategy for allocating new chunks of memory,
//! if need be we can replace the `Vec` we're using as a slab with something with a similar
//! allocation strategy. If memory usage is a problem we can compact `SimplifiedBoard`'s grid using
//! the `bitvec` crate. If allocator performance is still an issue, we could pool the bump
//! allocation arenas and the slab `Vec`s, but this seems unlikely.

use libtetris::{ Board, Piece, FallingPiece };
use std::collections::{ HashMap, VecDeque };
use smallvec::SmallVec;
use arrayvec::ArrayVec;
use enumset::EnumSet;
use enum_map::EnumMap;
use rand::prelude::*;
use crate::evaluation::Evaluation;

pub struct DagState<E: 'static, R: 'static> {
    board: Board,
    generations: VecDeque<Generation<E, R>>,
    root: u32,
    pieces: u32
}

enum Generation<E: 'static, R: 'static> {
    Known(rented::KnownGeneration<E, R>),
    Speculated(rented::SpeculatedGeneration<E, R>)
}

rental! {
    mod rented {
        #[rental]
        pub(super) struct KnownGeneration<E: 'static, R: 'static> {
            arena: Box<bumpalo::Bump>,
            data: super::KnownGeneration<'arena, E, R>
        }

        #[rental]
        pub(super) struct SpeculatedGeneration<E: 'static, R: 'static> {
            arena: Box<bumpalo::Bump>,
            data: super::SpeculatedGeneration<'arena, E, R>
        }
    }
}

struct KnownGeneration<'c, E, R> {
    nodes: Vec<Node<'c, E, R>>,
    deduplicator: HashMap<SimplifiedBoard<'c>, u32>,
    piece: Piece
}

struct SpeculatedGeneration<'c, E, R> {
    nodes: Vec<SpeculatedNode<'c, E, R>>,
    deduplicator: HashMap<SimplifiedBoard<'c>, u32>
}

struct Node<'c, E, R> {
    children: Option<&'c mut [Child<R>]>,
    parents: SmallVec<[u32; 4]>,
    evaluation: E,
    marked: bool
}

struct SpeculatedNode<'c, E, R> {
    children: Option<EnumMap<Piece, Option<&'c mut [Child<R>]>>>,
    parents: SmallVec<[u32; 4]>,
    evaluation: E,
}

struct Child<R> {
    placement: FallingPiece,
    accumulated: R,
    original_rank: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SimplifiedBoard<'c> {
    grid: &'c [u16],
    combo: u32,
    bag: EnumSet<Piece>,
    reserve: Piece,
    back_to_back: bool,
    reserve_is_hold: bool
}

impl<E: Evaluation<R> + 'static, R: 'static> DagState<E, R> {
    pub fn new(board: Board, use_hold: bool) -> Self {
        let mut generations = VecDeque::new();
        let mut next_pieces = board.next_queue();
        // if hold is enabled and hold is empty, we start the generations later than normal.
        if use_hold && board.hold_piece.is_none() {
            next_pieces.next();
        }
        for piece in next_pieces {
            generations.push_back(Generation::Known(rented::KnownGeneration::new(
                Box::new(bumpalo::Bump::new()),
                |_| KnownGeneration {
                    nodes: Vec::new(),
                    deduplicator: HashMap::new(),
                    piece
                }
            )));
        }
        generations.push_front(Generation::Known(rented::KnownGeneration::new(
            Box::new(bumpalo::Bump::new()),
            |_| KnownGeneration {
                nodes: vec![Node {
                    children: None,
                    parents: smallvec::SmallVec::new(),
                    evaluation: E::default(),
                    marked: false
                }],
                deduplicator: HashMap::new(),
                piece: Piece::I // nonsense piece; initial generation doesn't use it.
            }
        )));
        DagState {
            board,
            generations,
            root: 0,
            pieces: 0
        }
    }

    pub fn find_and_mark_leaf(
        &mut self, forced_analysis_lines: &mut Vec<Vec<FallingPiece>>
    ) -> Option<(NodeId, Board)> {
        let mut b = self.board.clone();
        let mut gen_index = 0;
        let mut node_key = self.root as usize;
        loop {
            let children = match &self.generations[gen_index] {
                Generation::Known(gen) => gen.maybe_ref_rent(
                    |gen| gen.nodes[node_key].children.as_ref().map(|c| &**c)
                ),
                Generation::Speculated(gen) => gen.maybe_ref_rent(|gen| {
                    let children = gen.nodes[node_key].children.as_ref()?;
                    let mut pick_from = ArrayVec::<[_; 7]>::new();
                    for (_, c) in children {
                        if let Some(c) = c {
                            if c.len() != 0 {
                                pick_from.push(&**c);
                            }
                        }
                    }
                    Some(*pick_from.choose(&mut thread_rng()).unwrap())
                })
            };
            // TODO pick child
        }

        // TODO return appropriate value
    }
}

pub struct NodeId {
    generation: u32,
    slab_key: u32
}