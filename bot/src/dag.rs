//! This file implements the DAG structure Cold Clear uses to store think results and facilitate the
//! Monte Carlo Tree Search. This is by far the most complicated file in Cold Clear.
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
//! cases. Note that we still need to know what the reserve piece is even when it's not the hold
//! piece due to speculation.
//! 
//! Visualization of the way generation pieces work:
//! ```plain
//! Gen piece: S        ( )ZST
//!                    /      \
//! Gen piece: T   (Z)T        ( )ST
//!               /    \      /     \
//! Speculated   (T)?  (Z)?  (S)?   ( )T?
//! ```
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
//! allocation strategy. If allocator performance is still an issue, we could pool the bump
//! allocation arenas and the slab `Vec`s, but this seems unlikely.
//! 
//! ## Some overall memory usage stuff
//! 
//! The hashmaps of `SimplifiedBoard`s will probably take up quite a bit of space, since a tetris
//! board is a large thing (400 cells!). We currently use the simple bitboard slice strategy. If
//! need be, we can easily switch to the compact bitboard slice strategy by using the `bitvec`
//! crate. I don't think my run-length encoding scheme saves enough memory to be worth the effort,
//! but I think it's cool. Quick overview of the memory usage of various strategies:
//! - Naive `[[bool; 10]; 40]`: 400 bytes
//! - Simple bitboard `[u16; 40]`: 80 bytes
//! - Compact bitboard `[u1; 400]`: 50 bytes
//! - Simple bitboard slice `&[u16]` (omit empty rows): 36 bytes-ish
//! - Compact bitboard slice `&[u1]` (omit trailing empty cells): 28 bytes-ish
//! - Run-length encoding scheme (using `&[u8]`): 32 bytes-ish
//! - Run-length encoding scheme (using `*const u8`): 25 bytes-ish
//! The "-ish" values are based on this board, which I assume is typical: http://fumen.zui.jp/?v115@BgA8CeA8EeA8BeD8CeF8CeH8AeK8AeI8AeI8AeE8Ae?I8AeI8AeD8JeAgH
//! 
//! My run-length encoding scheme is not self-explanatory, so I will describe it here (despite the
//! lack of any implementation). Represent the cells as a bitstring ordered as column 0 going up,
//! column 1 going down, column 2 going up, column 3 going down, etc. Represent each run as a byte
//! where the topmost bit is 1 if the run is of filled cells and 0 if it is of empty cells. The
//! remaining 7 bits store the length of the run minus 1. If a run is longer than 128, it is
//! transformed to a run of length 128 plus a run of the remaining length. Since the length of the
//! byte sequence is represented in the data, the pointer can be made thin, but this requires the
//! use of `unsafe` code when decoding.
//! 
//! Example 1: the empty field is represented as `0x7F 0x7F 0x7F 0x0F`; three runs of empty cells of
//! length 128, and a run of empty cells of length 16. Example 2: the field containing only an I
//! piece laid flat in the center is represented as `0x7F 0x1E 0x81 0x4D 0x81 0x7F 0x1E`.
#![allow(dead_code)]

use libtetris::{ Board, Piece, FallingPiece, LockResult };
use std::collections::{ HashMap, VecDeque };
use arrayvec::ArrayVec;
use enumset::EnumSet;
use enum_map::EnumMap;
use serde::{ Serialize, Deserialize };
use rand::prelude::*;
use bumpalo::collections::vec::Vec as BumpVec;
use crate::evaluation::Evaluation;

pub struct DagState<E: 'static, R: 'static> {
    board: Board,
    generations: VecDeque<rented::Generation<E, R>>,
    root: u32,
    gens_passed: u32
}

#[derive(Serialize, Deserialize)]
pub struct NodeId {
    generation: u32,
    slab_key: u32
}

#[derive(Serialize, Deserialize)]
pub struct ChildData<E, R> {
    pub mv: FallingPiece,
    pub board: Board,
    pub evaluation: E,
    pub reward: R,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveCandidate<E> {
    pub mv: FallingPiece,
    pub lock: LockResult,
    pub board: Board,
    pub evaluation: E,
    pub hold: bool,
    pub original_rank: u32
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
    nodes: Vec<Node<'c, E>>,
    children: Children<'c, R>,
    deduplicator: HashMap<SimplifiedBoard<'c>, u32>,
}

enum Children<'c, R> {
    // we need to know the piece to resolve speculations computed before a new piece was added,
    // but given to us after a new piece was added.
    Known(Piece, Vec<Option<&'c mut [Child<R>]>>),
    Speculated(Vec<Option<EnumMap<Piece, Option<&'c mut [Child<R>]>>>>)
}

struct Node<'c, E> {
    parents: BumpVec<'c, u32>,
    evaluation: E,
    nodes: u32,
    marked: bool,
    death: bool,
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
        // if hold is enabled and hold is empty, the generation piece is later than normal.
        if use_hold && board.hold_piece.is_none() {
            next_pieces.next().expect("Not enough next pieces provided to initialize");
        }
        generations.push_back(rented::Generation::new(
            Box::new(bumpalo::Bump::new()),
            |bump| Generation {
                nodes: vec![Node {
                    parents: BumpVec::new_in(bump),
                    evaluation: E::default(),
                    nodes: 1,
                    marked: false,
                    death: false
                }],
                children: match next_pieces.next() {
                    Some(p) => Children::Known(p, vec![None]),
                    None => Children::Speculated(vec![None])
                },
                // nothing new will ever be put in the root generation, so we won't bother to
                // put anything in the hashmap.
                deduplicator: HashMap::new()
            }
        ));
        // initialize the remaining known generations
        for piece in next_pieces {
            generations.push_back(rented::Generation::known(piece));
        }
        DagState {
            board,
            generations,
            root: 0,
            gens_passed: 0
        }
    }

    pub fn find_and_mark_leaf(
        &mut self, forced_analysis_lines: &mut Vec<Vec<FallingPiece>>
    ) -> Option<(NodeId, Board)> {
        for i in (0..forced_analysis_lines.len()).rev() {
            // Attempt to search forced lines first
            let mut path = &*forced_analysis_lines[i];
            let mut done = false;
            let choice = self.find_and_mark_leaf_with_chooser(|_, children| {
                if let &[next, ref rest @ ..] = path {
                    for child in children {
                        if next.same_location(&child.placement) {
                            // found the next step on the path; traverse
                            if rest.is_empty() {
                                // this is last step on path, so we're done with it
                                done = true;
                            }
                            path = rest;
                            return Some(child)
                        }
                    }
                }
                // either there isn't a next step on the path or we failed to find a child on the
                // next step of the path. In both cases we're done with the path and cease searching
                done = true;
                None
            });
            if done {
                forced_analysis_lines.swap_remove(i);
            }
            if choice.is_some() {
                return choice;
            }
        }

        self.find_and_mark_leaf_with_chooser(|next_gen_nodes, children| {
            // This returns None for death, and the raw evaluation otherwise.
            let evaluation = |c: &Child<R>| {
                let node = &next_gen_nodes[c.node as usize];
                if node.death {
                    None
                } else {
                    Some(node.evaluation.clone() + c.reward.clone())
                }
            };
            // Since children is sorted best-to-worst, the minimum evaluation will be the last item
            // in the iterator. filter_map allows us to ignore death nodes.
            let min_eval = children.iter().rev().filter_map(evaluation).next()?;
            let weights = children.iter().enumerate().map(
                |(i, c)| evaluation(c).map_or(0,
                    |e| e.weight(&min_eval, i)
                )
            );
            // Choose a node randomly (the Monte-Carlo part)
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
            // Get the list of childs of the current node, or None if this is a leaf
            let children = self.generations[gen_index].maybe_ref_rent(|gen| match &gen.children {
                Children::Known(_, childrens) => childrens[node_key].as_deref(),
                Children::Speculated(childrens) => {
                    // We must select a single group of children to search further. We do this by
                    // finding the set of valid next pieces and randomly selecting one uniformly.
                    // We then take the group of children associated with that piece.
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

            if let Some(children) = children {
                // Branch case. Call the chooser to pick the branch to take.
                self.generations[gen_index+1].rent(|gen| {
                    let child = chooser(
                        &gen.nodes,
                        children
                    )?;
                    advance(&mut board, child.placement);
                    gen_index += 1;
                    node_key = child.node as usize;
                    Some(())
                })?;
            } else if self.generations[gen_index].rent(|gen| gen.nodes[node_key].marked) {
                // this leaf has already been returned for processing, so it's not valid
                return None
            } else {
                // found a valid leaf, so mark it and return it
                self.generations[gen_index].rent_mut(|gen| gen.nodes[node_key].marked = true);
                return Some((NodeId {
                    generation: gen_index as u32 + self.gens_passed,
                    slab_key: node_key as u32
                }, board))
            }
        }
    }

    pub fn update_known(&mut self, node: NodeId, children: Vec<ChildData<E, R>>) {
        // make sure we weren't given a NodeId for an expired node. it could happen.
        if node.generation < self.gens_passed {
            return
        }
        let gen = (node.generation - self.gens_passed) as usize;

        let [parent_gen, child_gen] = self.get_gen_and_next(gen);

        parent_gen.rent_all_mut(|current| child_gen.rent_all_mut(|mut next|
            match &mut current.data.children {
                Children::Known(_, c) => c[node.slab_key as usize] = Some(build_children(
                    current.arena, &mut next, children, node.slab_key
                )),
                Children::Speculated(_) => unreachable!()
            }
        ));

        self.backpropogate(gen, node.slab_key as usize);
    }

    pub fn update_speculated(
        &mut self, node: NodeId,
        mut children: EnumMap<Piece, Option<Vec<ChildData<E, R>>>>
    ) {
        // make sure we weren't given a NodeId for an expired node. it could happen.
        if node.generation < self.gens_passed {
            return
        }
        let gen = (node.generation - self.gens_passed) as usize;

        let [parent_gen, child_gen] = self.get_gen_and_next(gen);

        parent_gen.rent_all_mut(|current| child_gen.rent_all_mut(|mut next|
            match &mut current.data.children {
                // Deal with the case that the generation has been resolved 
                Children::Known(piece, c) => c[node.slab_key as usize] = Some(build_children(
                    current.arena,
                    &mut next,
                    children[*piece].take().unwrap_or_else(|| speculation_broke(*piece, &children)),
                    node.slab_key
                )),
                Children::Speculated(c) => {
                    let mut childs = EnumMap::new();
                    for (p, data) in children {
                        if let Some(data) = data {
                            childs[p] = Some(build_children(
                                current.arena, &mut next, data, node.slab_key
                            ));
                        }
                    }
                    c[node.slab_key as usize] = Some(childs);
                }
            }
        ));

        self.backpropogate(gen, node.slab_key as usize);
    }

    fn backpropogate(&mut self, gen: usize, node: usize) {
        let mut to_update = VecDeque::new();
        to_update.push_back((gen, node));

        while let Some((gen, node)) = to_update.pop_front() {
            let [parent_gen, children_gen] = self.get_gen_and_next(gen);
            parent_gen.rent_mut(|parent_gen| children_gen.rent(|children_gen| {
                // TODO
            }));
        }
    }

    fn get_gen_and_next(&mut self, gen: usize) -> [&mut rented::Generation<E, R>; 2] {
        if gen == self.generations.len() - 1 {
            // we're expanding into boards that belong in a generation that doesn't exist yet.
            // since it doesn't exist, we're missing some next queue information, so it's a
            // speculated generation.
            self.generations.push_back(rented::Generation::speculated());
        }

        fn get2_mut<T>(s: &mut [T], i: usize) -> [&mut T; 2] {
            let (a, b) = s.split_at_mut(i+1);
            [&mut a[a.len() - 1], &mut b[0]]
        }

        // need to do something a little weird to get mutable references to both generations
        let (a, b) = self.generations.as_mut_slices();
        if gen + 1 == a.len() {
            [&mut a[gen], &mut b[0]]
        } else if gen < a.len() {
            get2_mut(a, gen)
        } else {
            get2_mut(b, gen - a.len())
        }
    }

    pub fn unmark(&mut self, node: NodeId) {
        // make sure we weren't given a NodeId for an expired node. it could happen.
        if node.generation >= self.gens_passed {
            self.generations[(node.generation - self.gens_passed) as usize].rent_mut(
                |gen| gen.nodes[node.slab_key as usize].marked = false
            );
        }
    }

    pub fn add_next_piece(&mut self, piece: Piece) {
        self.board.add_next_piece(piece);
        // resolve a speculated generation if possible
        for gen in &mut self.generations {
            let done = gen.rent_mut(|gen| match &mut gen.children {
                Children::Speculated(childs) => {
                    let mut newchildren = vec![];
                    for child in std::mem::take(childs) {
                        newchildren.push(child.map(|mut cases|
                            std::mem::take(&mut cases[piece]).unwrap_or_else(
                                || speculation_broke(piece, &cases)
                            )
                        ));
                    }
                    gen.children = Children::Known(piece, newchildren);
                    true
                }
                _ => false
            });
            if done { return }
        }
        // if are no speculated generations, add a known generation
        self.generations.push_back(rented::Generation::known(piece));
    }

    pub fn get_plan(&self) -> Vec<(FallingPiece, LockResult)> {
        let mut node = self.root;
        let mut plan = vec![];
        let mut board = self.board.clone();
        for gen in &self.generations {
            let done = gen.rent(|gen| match &gen.children {
                Children::Known(_, c) => match c[node as usize].as_ref().and_then(|c| c.first()) {
                    Some(child) => {
                        plan.push((child.placement, advance(&mut board, child.placement)));
                        node = child.node;
                        false
                    }
                    None => true
                }
                _ => true
            });
            if done { break }
        }
        plan
    }

    pub fn reset(&mut self, field: [[bool; 10]; 40], b2b: bool, combo: u32) -> Option<i32> {
        let garbage_lines;
        if b2b == self.board.b2b_bonus && combo == self.board.combo {
            let mut b = Board::<u16>::new();
            b.set_field(field);
            let dif = self.board.column_heights().iter()
                .zip(b.column_heights().iter())
                .map(|(&y1, &y2)| y2 - y1)
                .min().unwrap();
            let mut is_garbage_receive = true;
            for y in 0..(40 - dif) {
                if b.get_row(y + dif) != self.board.get_row(y) {
                    is_garbage_receive = false;
                    break;
                }
            }
            if is_garbage_receive {
                garbage_lines = Some(dif);
            } else {
                garbage_lines = None;
            }
        } else {
            garbage_lines = None;
        }

        self.board.set_field(field);
        self.board.combo = combo;
        self.board.b2b_bonus = b2b;

        self.gens_passed += self.generations.len() as u32 + 1;
        self.root = 0;
        self.generations.clear();
        self.generations.push_back(rented::Generation::new(
            Box::new(bumpalo::Bump::new()),
            |bump| Generation {
                nodes: vec![Node {
                    parents: BumpVec::new_in(bump),
                    evaluation: E::default(),
                    nodes: 1,
                    marked: false,
                    death: false
                }],
                children: Children::Known(Piece::I, vec![None]), // nonsense piece never used
                deduplicator: HashMap::new() // nothing else will ever be put in this generation
            }
        ));

        garbage_lines
    }

    pub fn get_next_candidates(&self) -> Vec<MoveCandidate<E>> {
        self.generations[0].rent(|gen| {
            let mut candidates = vec![];
            if let Children::Known(_, children) = &gen.children {
                if let Some(children) = &children[self.root as usize] {
                    for (i, child) in children.iter().enumerate() {
                        let mut board = self.board.clone();
                        let lock = advance(&mut board, child.placement);
                        let eval = self.generations[1].rent(
                            |gen| gen.nodes[child.node as usize].evaluation.clone()
                        );
                        candidates.push(MoveCandidate {
                            mv: child.placement,
                            hold: self.board.hold_piece != board.hold_piece,
                            evaluation: eval + child.reward.clone(),
                            original_rank: i as u32,
                            lock, board,
                        });
                    }
                }
            }
            candidates
        })
    }

    pub fn advance_move(&mut self, mv: FallingPiece) {
        let new_root = self.generations[0].rent(|gen|
            if let Children::Known(_, children) = &gen.children {
                children[self.root as usize].as_ref().and_then(
                    |children| children.iter().find(|c| c.placement == mv)
                ).map(|c| c.node)
            } else {
                None
            }
        ).expect("An invalid move was chosen");

        self.root = new_root;
        advance(&mut self.board, mv);
        self.generations.pop_front();
    }

    pub fn nodes(&self) -> u32 {
        self.generations[0].rent(|gen| gen.nodes[self.root as usize].nodes)
    }

    pub fn depth(&self) -> u32 {
        self.generations.len() as u32 - 1
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn is_dead(&self) -> bool {
        self.generations[0].rent(|gen| match &gen.children {
            Children::Known(_, childrens) =>
                childrens[self.root as usize].as_ref().map_or(false, |s| s.is_empty()),
            Children::Speculated(childrens) =>
                childrens[self.root as usize].as_ref().map_or(false, |s| s.is_empty()),
        })
    }
}

/// keeps queue state consistent while arbitrarily placing pieces
fn advance(board: &mut Board, placement: FallingPiece) -> LockResult {
    let result = board.lock_piece(placement);
    let next = board.advance_queue().unwrap();
    if next != placement.kind.0 {
        let unheld = board.hold(next);
        let p = unheld.unwrap_or_else(|| board.advance_queue().unwrap());
        assert_eq!(p, placement.kind.0);
    }
    result
}

fn build_children<'arena, E: Evaluation<R> + 'static, R: Clone + 'static>(
    parent_arena: &'arena bumpalo::Bump,
    children_gen: &mut rented::Generation_BorrowMut<E, R>,
    mut children: Vec<ChildData<E, R>>,
    parent: u32
) -> &'arena mut [Child<R>] {
    // sort best to worst
    children.sort_by_key(
        |c| std::cmp::Reverse(c.evaluation.clone() + c.reward.clone())
    );
    parent_arena.alloc_slice_fill_iter(children.into_iter().enumerate().map(
        |(i, data)| {
            // this arrayvec will almost always be shorter than 40 elements,
            // since it won't store the upper empty rows. this is to save memory.
            let mut simple_grid = ArrayVec::<[_; 40]>::new();
            let terrain_height = data.board.column_heights().iter().copied().max().unwrap();
            for y in 0..terrain_height {
                simple_grid.push(*data.board.get_row(y));
            }

            let simple_board = SimplifiedBoard {
                grid: &simple_grid,
                back_to_back: data.board.b2b_bonus,
                combo: data.board.combo,
                bag: data.board.next_bag(),
                reserve: data.board.hold_piece.unwrap_or_else(
                    || data.board.next_queue().next().unwrap()
                ),
                reserve_is_hold: data.board.hold_piece.is_some()
            };

            // check if the board is duplicated
            let node = match children_gen.data.deduplicator.get(&simple_board) {
                Some(&node) => node,
                None => {
                    // new board; create node, children, deduplicator entry
                    let node = children_gen.data.nodes.len();
                    children_gen.data.nodes.push(Node {
                        parents: BumpVec::new_in(&children_gen.arena),
                        evaluation: data.evaluation,
                        nodes: 1,
                        death: false,
                        marked: false
                    });
                    match &mut children_gen.data.children {
                        Children::Known(_, children) => children.push(None),
                        Children::Speculated(children) => children.push(None)
                    }
                    children_gen.data.deduplicator.insert(SimplifiedBoard {
                        grid: children_gen.arena.alloc_slice_copy(&simple_grid),
                        ..simple_board
                    }, node as u32);
                    node as u32
                }
            };
            children_gen.data.nodes[node as usize].parents.push(parent);

            Child {
                placement: data.mv,
                original_rank: i as u32,
                reward: data.reward,
                node
            }
        }
    ))
}

impl<E: 'static, R: 'static> rented::Generation<E, R> {
    pub fn known(piece: Piece) -> Self {
        rented::Generation::new(
            Box::new(bumpalo::Bump::new()),
            |_| Generation {
                nodes: vec![],
                deduplicator: HashMap::new(),
                children: Children::Known(piece, vec![])
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

fn speculation_broke<T>(piece: Piece, cases: &EnumMap<Piece, Option<T>>) -> ! {
    let mut expected = ArrayVec::<[_; 7]>::new();
    for (p, v) in cases {
        if v.is_some() {
            expected.push(p);
        }
    }
    panic!("speculation broke; expected {:?}, got {:?}", expected, piece)
}