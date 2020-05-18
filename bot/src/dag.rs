//! This file implements the DAG structure Cold Clear uses to store think results and facilitate the
//! Monte Carlo Tree Search. The is by far the most complicated file in Cold Clear.
//! 
//! ## Implementation notes
//! 
//! A generation contains all of the nodes that involve placing a specific number of pieces on
//! the board. `DagState.node_generations[0]` is always the generation of the current board state
//! and always belongs to generation `DagState.pieces`. When a move is picked, the generation the
//! root node previously belonged to is deleted.
//! 
//! Generations are associated with pieces in the next queue in a natural way. If the associated
//! piece is not known, the generation is a speculated generation. It follows that there are never
//! known generations following the first speculated generation. When a new piece is added to the
//! next queue, the associated generation is converted from speculated to known. This process does
//! not change the slab keys of nodes in the converted generation so that links in the prior and
//! next generation aren't invalidated.
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
//! Traversing the DAG requires cloning the `DagState.board` and calling `Board::lock_piece` every
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
    generations: VecDeque<rented::Generation<E, R>>,
    root: u32,
    pieces: u32
}

rental! {
    mod rented {
        #[rental]
        pub(super) struct Generation<E: 'static, R: 'static> {
            arena: Box<bumpalo::Bump>,
            data: super::Generation<'arena, E, R>
        }
    }
}

struct Generation<'c, E, R> {
    nodes: Vec<Node<E>>,
    children: Children<'c, R>,
    deduplicator: HashMap<SimplifiedBoard<'c>, u32>,
}

enum Children<'c, R> {
    Known(Vec<Option<&'c mut [Child<R>]>>),
    Speculated(Vec<Option<EnumMap<Piece, Option<&'c mut [Child<R>]>>>>)
}

struct Node<E> {
    parents: SmallVec<[u32; 4]>,
    evaluation: E,
    marked: bool,
    death: bool
}

struct Child<R> {
    placement: FallingPiece,
    reward: R,
    original_rank: u32,
    node: u32
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

impl<E: Evaluation<R> + 'static, R: Clone + 'static> DagState<E, R> {
    pub fn new(board: Board, use_hold: bool) -> Self {
        let mut generations = VecDeque::new();
        let mut next_pieces = board.next_queue();
        // if hold is enabled and hold is empty, we start the generations later than normal.
        if use_hold && board.hold_piece.is_none() {
            next_pieces.next();
        }
        for piece in next_pieces {
            generations.push_back(rented::Generation::known());
        }
        let root_generation = rented::Generation::new(
            Box::new(bumpalo::Bump::new()),
            |_| Generation {
                nodes: vec![Node {
                    parents: SmallVec::new(),
                    evaluation: E::default(),
                    marked: false,
                    death: false
                }],
                children: Children::Known(vec![None]),
                deduplicator: HashMap::new() // nothing else will ever be put in this generation
            }
        );
        generations.push_front(root_generation);
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
        for i in (0..forced_analysis_lines.len()).rev() {
            let mut path = &*forced_analysis_lines[i];
            let mut done = false;
            let choice = self.find_and_mark_leaf_with_chooser(|_, children| match path {
                &[] => {
                    // already analysed this path, so we're done with it
                    done = true;
                    None
                }
                &[next, ref rest@..] => {
                    let mut target = next.cells();
                    target.sort();
                    for child in children {
                        let mut cells = child.placement.cells();
                        cells.sort();
                        if cells == target {
                            // found the next step on the path
                            if rest.is_empty() {
                                // this is last step on path, so we're done with it
                                done = true;
                            }
                            path = rest;
                            return Some(child)
                        }
                    }
                    // can't find the next step on the path, so we're done with it
                    done = true;
                    None
                }
            });
            if done {
                forced_analysis_lines.swap_remove(i);
            }
            if choice.is_some() {
                return choice;
            }
        }

        self.find_and_mark_leaf_with_chooser(|next_gen_nodes, children| {
            // Pick non-death nodes in a weighted-random fashion (the Monte Carlo part)
            let evaluation = |c: &Child<R>| {
                let node = &next_gen_nodes[c.node as usize];
                if node.death {
                    None
                } else {
                    Some(node.evaluation.clone() + c.reward.clone())
                }
            };
            let min_eval = children.iter().filter_map(evaluation).min()?;
            let weights = children.iter().enumerate()
                .map(|(i, c)| evaluation(c).map_or(0, |e| e.weight(&min_eval, i)));
            let sampler = rand::distributions::WeightedIndex::new(weights).ok()?;
            Some(&children[thread_rng().sample(sampler)])
        })
    }

    fn find_and_mark_leaf_with_chooser(
        &mut self,
        mut chooser: impl for<'a> FnMut(&[Node<E>], &'a [Child<R>]) -> Option<&'a Child<R>>
    ) -> Option<(NodeId, Board)> {
        let mut board = self.board.clone();
        let mut gen_index = 0;
        let mut node_key = self.root as usize;
        loop {
            let node = self.generations[gen_index].ref_rent(|gen| &gen.nodes[node_key]);
            let children = self.generations[gen_index].maybe_ref_rent(|gen| match &gen.children {
                Children::Known(childrens) => childrens[node_key].as_deref(),
                Children::Speculated(childrens) => {
                    // We must select a single group of children to search further. We do this by
                    // randomly selecting a valid next piece and using the child group associated
                    // with that piece.
                    let children = childrens[node_key].as_ref()?;
                    let mut pick_from = ArrayVec::<[_; 7]>::new();
                    for (p, c) in children {
                        if let Some(c) = c {
                            pick_from.push((p, &**c));
                        }
                    }
                    let (piece, children) = *pick_from.choose(&mut thread_rng()).unwrap();
                    board.add_next_piece(piece);
                    Some(children)
                }
            });

            let children = match children {
                // branch
                Some(v) => v,
                // leaf
                None => if node.marked {
                    // this node has already been returned for processing, so we failed to find
                    // a leaf.
                    return None
                } else {
                    // found a valid leaf, so mark it and return
                    self.generations[gen_index].rent_mut(|gen| gen.nodes[node_key].marked = true);
                    return Some((NodeId {
                        generation: gen_index as u32 + self.pieces,
                        slab_key: node_key as u32
                    }, board))
                }
            };

            let child = chooser(
                self.generations[gen_index+1].ref_rent(|gen| &gen.nodes),
                children
            )?;
            advance(&mut board, child.placement);
            gen_index += 1;
            node_key = child.node as usize;
        }
    }
}

/// keeps queue state consistent while arbitrarily placing pieces
fn advance(board: &mut Board, placement: FallingPiece) {
    board.lock_piece(placement);
    let next = board.advance_queue().unwrap();
    if next != placement.kind.0 {
        let unheld = board.hold(next);
        let p = unheld.unwrap_or_else(|| board.advance_queue().unwrap());
        assert_eq!(p, placement.kind.0);
    }
}

impl<E: 'static, R: 'static> rented::Generation<E, R> {
    pub fn known() -> Self {
        rented::Generation::new(
            Box::new(bumpalo::Bump::new()),
            |_| Generation {
                nodes: vec![],
                deduplicator: HashMap::new(),
                children: Children::Known(vec![])
            }
        )
    }

    pub fn speculated() -> Self {
        rented::Generation::new(
            Box::new(bumpalo::Bump::new()),
            |_| Generation {
                nodes: vec![],
                deduplicator: HashMap::new(),
                children: Children::Speculated(vec![])
            }
        )
    }
}

pub struct NodeId {
    generation: u32,
    slab_key: u32
}