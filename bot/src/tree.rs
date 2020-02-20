use std::collections::{ VecDeque, HashMap, HashSet };
use libtetris::{ Piece, FallingPiece, LockResult, Board };
use arrayvec::ArrayVec;
use smallvec::SmallVec;
use enumset::EnumSet;
use enum_map::EnumMap;
use rand::prelude::*;
use crate::evaluation::Evaluation;

pub struct TreeState<E, R> {
    pub board: Board,
    root: usize,
    boards: HashMap<SimplifiedBoard, usize>,
    trees: Vec<Tree<E>>,
    children: Vec<Option<Children>>,
    childs: Vec<Child<R>>,
    backbuffer_trees: Vec<Tree<E>>,
    backbuffer_children: Vec<Option<Children>>,
    backbuffer_childs: Vec<Child<R>>,
    next_speculation: HashSet<usize>,
    pieces: Pieces,
    use_hold: bool,
    pub nodes: usize,
    generation: u32
}

struct Pieces {
    piece_queue: VecDeque<Piece>,
    pieces_used: u32
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(u32, usize);

#[derive(Clone)]
struct Tree<E> {
    board: SimplifiedBoard,
    parents: SmallVec<[usize; 4]>,
    depth: usize,
    evaluation: E,
    marked: bool,
    death: bool
}

pub struct ChildData<E, R> {
    pub mv: FallingPiece,
    pub lock: LockResult,
    pub board: Board,
    pub accumulated: R,
    pub evaluation: E,
    pub hold: bool
}

pub struct MoveCandidate<E> {
    pub mv: FallingPiece,
    pub lock: LockResult,
    pub board: Board,
    pub evaluation: E,
    pub hold: bool,
    pub original_rank: usize
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Child<R> {
    pub mv: FallingPiece,
    pub lock: LockResult,
    pub node: usize,
    pub original_rank: usize,
    accumulated: R,
    pub hold: bool
}

pub enum Children {
    Known(usize, usize),
    Speculation(EnumMap<Piece, Option<(usize, usize)>>)
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SimplifiedBoard {
    grid: ArrayVec<[u16; 40]>,
    pieces_used: u32,
    combo: u32,
    bag: EnumSet<Piece>,
    reserve: Piece,
    reserve_is_hold: bool,
    back_to_back: bool,
}

impl<E: Evaluation<R>, R: Clone> TreeState<E, R> {
    /// Requires that there is at least one next piece if `use_hold` is true.
    pub fn create(board: Board, use_hold: bool) -> Self {
        let b = board.clone();
        let mut this = TreeState {
            root: 0,
            trees: Vec::with_capacity(2_000_000),
            children: Vec::with_capacity(2_000_000),
            childs: Vec::with_capacity(2_000_000),
            backbuffer_trees: Vec::with_capacity(2_000_000),
            backbuffer_children: Vec::with_capacity(2_000_000),
            backbuffer_childs: Vec::with_capacity(2_000_000),
            next_speculation: HashSet::new(),
            boards: HashMap::new(),
            pieces: Pieces {
                piece_queue: board.next_queue().collect(),
                pieces_used: 0
            },
            board,
            use_hold,
            generation: 0,
            nodes: 0
        };
        let sb = this.to_simplified_board(&b, if use_hold { 1 } else { 0 });
        this.root = this.create_tree(Tree {
            board: sb,
            parents: SmallVec::new(),
            evaluation: E::default(),
            depth: 0,
            marked: false,
            death: false
        });
        this
    }

    pub fn reset(&mut self, field: [[bool; 10]; 40], b2b: bool, combo: u32) {
        self.board.set_field(field);
        self.board.combo = combo;
        self.board.b2b_bonus = b2b;

        self.boards.clear();
        self.next_speculation.clear();
        self.trees.clear();
        self.children.clear();
        self.childs.clear();
        self.generation += 1;
        self.nodes = 0;

        let pieces_used = if self.use_hold && self.board.hold_piece.is_none() {
            self.pieces.pieces_used + 1
        } else {
            self.pieces.pieces_used
        };
        let sb = self.to_simplified_board(&self.board, pieces_used);
        self.root = self.create_tree(Tree {
            board: sb,
            parents: SmallVec::new(),
            evaluation: E::default(),
            depth: 0,
            marked: false,
            death: false
        });
    }

    /// To be called by a worker looking to expand the tree. `update_known`, `update_speculated`, or
    /// `unmark` should be called to provide the generated children. If this returns `None`, the
    /// leaf found is already being expanded by another worker, and you should try again later.
    pub fn find_and_mark_leaf(&mut self) -> Option<(NodeId, Board)> {
        if self.is_dead() {
            return None
        }
        let mut current = 0;
        loop {
            match self.children[current] {
                None => {
                    if self.trees[current].marked {
                        return None
                    } else {
                        self.trees[current].marked = true;
                        return Some((
                            NodeId(self.generation, current),
                            self.pieces.rebuild_board(&self.trees[current].board)
                        ));
                    }
                },
                Some(Children::Known(start, len)) =>
                    current = pick(&self.trees, &self.childs[start..start+len]),
                Some(Children::Speculation(c)) => {
                    let mut pick_from = ArrayVec::<[_; 7]>::new();
                    for (_, c) in c {
                        if let Some(c) = c {
                            if c.1 != 0 {
                                pick_from.push(c);
                            }
                        }
                    }
                    let &(start, len) = pick_from.choose(&mut thread_rng()).unwrap();
                    current = pick(&self.trees, &self.childs[start..start+len]);
                }
            }
        }
    }

    /// To be called when a worker has generated and evaluated the children of the node.
    pub fn update_known(&mut self, node: NodeId, children: Vec<ChildData<E, R>>) {
        if node.0 != self.generation {
            // Since a move can be taken between find_leaf and update_whatever calls,
            // we need to check if that's happened. It's possible that the specified node already
            // exists, but it's easier to just drop the result and recalculate later.
            return
        }

        let (start, len) = self.build_children(node.1, children);
        self.children[node.1] = Some(Children::Known(start, len));
        self.trees[node.1].marked = false;

        let mut v = VecDeque::new();
        v.push_back(node.1);
        self.update(v);
    }

    /// To be called when a worker has generated and evaluated the children of the node.
    pub fn update_speculated(
        &mut self, node: NodeId, mut children: EnumMap<Piece, Option<Vec<ChildData<E, R>>>>
    ) {
        if node.0 != self.generation {
            return
        }

        let speculation_piece_index = (
            self.trees[node.1].board.pieces_used - self.pieces.pieces_used
        ) as usize;

        if speculation_piece_index < self.pieces.piece_queue.len() {
            // A next piece was previously added that resolves this speculation.
            let mut c = vec![];
            // this unwrap is okay because a next piece not in the bag can't be added.
            std::mem::swap(
                &mut c,
                children[self.pieces.piece_queue[speculation_piece_index]].as_mut().unwrap()
            );
            self.update_known(node, c);
            return;
        }

        if speculation_piece_index == self.pieces.piece_queue.len() {
            // Next speculation (will be resolved with the next piece)
            self.next_speculation.insert(node.1);
        }
        let mut childs = EnumMap::new();
        for (p, c) in children {
            if let Some(c) = c {
                childs[p] = Some(self.build_children(node.1, c));
            }
        }
        self.children[node.1] = Some(Children::Speculation(childs));
        self.trees[node.1].marked = false;

        let mut v = VecDeque::new();
        v.push_back(node.1);
        self.update(v);
    }

    pub fn unmark(&mut self, node: NodeId) {
        if node.0 != self.generation {
            return
        }
        self.trees[node.1].marked = false;
    }

    /// Adds the next piece and resolves the affected speculation nodes.
    pub fn add_next_piece(&mut self, piece: Piece) {
        self.pieces.piece_queue.push_back(piece);
        self.board.add_next_piece(piece);
        let mut next_speculation = HashSet::new();
        let mut to_update = VecDeque::new();
        std::mem::swap(&mut self.next_speculation, &mut next_speculation);
        for node in next_speculation {
            let childs = self.children[node].as_ref().unwrap();
            let (start, len) = if let Children::Speculation(possibilities) = childs {
                match possibilities[piece] {
                    Some(v) => v,
                    None => {
                        println!("speculation machine broke");
                        (0, 0)
                    }
                }
            } else {
                unreachable!()
            };
            self.children[node] = Some(Children::Known(start, len));
            let children = &self.childs[start .. start+len];
            for child in children {
                if self.children[child.node].is_some() {
                    self.next_speculation.insert(child.node);
                }
            }
            to_update.push_back(node);
        }
        self.update(to_update);
    }

    /// Retrieve the best next moves, sorted from best to worst.
    pub fn get_next_candidates(&self) -> Vec<MoveCandidate<E>> {
        if let Some(Children::Known(start, len)) = self.children[self.root] {
            self.childs[start..start+len].iter()
                .map(|c| MoveCandidate {
                    board: self.pieces.rebuild_board(&self.trees[c.node].board),
                    lock: c.lock.clone(),
                    evaluation: self.trees[c.node].evaluation.clone() + c.accumulated.clone(),
                    hold: c.hold,
                    mv: c.mv,
                    original_rank: c.original_rank
                })
                .collect()
        } else {
            vec![]
        }
    }

    pub fn get_plan(&self) -> Vec<(FallingPiece, LockResult)> {
        let mut plan = vec![];
        let mut node = self.root;
        while let &Some(Children::Known(start, len)) = &self.children[node] {
            if len == 0 {
                break
            }
            let child = &self.childs[start];
            plan.push((child.mv, child.lock.clone()));
            node = child.node;
        }
        plan
    }

    pub fn advance_move(&mut self, mv: FallingPiece) {
        let child = if let Some(Children::Known(start, len)) = self.children[self.root] {
            self.childs[start..start+len].iter()
                .find(|c| c.mv == mv)
                .expect("Tried to do a move that can't be done")
        } else {
            panic!("Not enough thinking or not enough next pieces to advance a move");
        };
        self.root = child.node;
        let next = self.board.advance_queue().unwrap();
        if child.hold {
            if self.board.hold(next).is_none() {
                self.board.advance_queue().unwrap();
                self.pieces.pieces_used += 1;
                self.pieces.piece_queue.pop_front();
            }
        }
        self.board.lock_piece(child.mv);
        self.pieces.pieces_used += 1;
        self.pieces.piece_queue.pop_front();

        self.gc();
    }

    pub fn is_dead(&self) -> bool {
        match &self.children[self.root] {
            Some(children) => children.is_dead(),
            None => false
        }
    }

    pub fn depth(&self) -> usize {
        self.trees[self.root].depth
    }

    fn build_children(
        &mut self, parent: usize, mut children: Vec<ChildData<E, R>>
    ) -> (usize, usize) {
        let pieces_used = self.trees[parent].board.pieces_used;
        children.sort_by_key(|c| std::cmp::Reverse(c.evaluation.clone() + c.accumulated.clone()));
        let start = self.childs.len();
        for (i, data) in children.into_iter().enumerate() {
            let node = self.make_node(
                self.to_simplified_board(&data.board, pieces_used+1),
                parent, data.evaluation
            );
            self.childs.push(Child {
                mv: data.mv,
                lock: data.lock,
                hold: data.hold,
                original_rank: i,
                accumulated: data.accumulated,
                node
            });
        }
        (start, self.childs.len() - start)
    }

    fn make_node(&mut self, board: SimplifiedBoard, parent: usize, eval: E) -> usize {
        use std::collections::hash_map::Entry;
        match self.boards.entry(board.clone()) {
            Entry::Occupied(entry) => {
                let &id = entry.get();
                self.trees[id].parents.push(parent);
                id
            }
            Entry::Vacant(entry) => {
                let tree = Tree {
                    board,
                    parents: SmallVec::from_elem(parent, 1),
                    evaluation: eval,
                    depth: 0,
                    marked: false,
                    death: false
                };
                let index = self.trees.len();
                entry.insert(index);
                self.trees.push(tree);
                self.children.push(None);
                self.nodes += 1;
                index
            }
        }
    }

    fn update(&mut self, mut to_update: VecDeque<usize>) {
        while let Some(node) = to_update.pop_front() {
            match self.children[node].as_mut().unwrap() {
                Children::Known(start, len) => {
                    // We may have discovered some paths result in death, so remove those
                    let mut i = *start;
                    while i < *start+*len {
                        if self.trees[self.childs[i].node].death {
                            *len -= 1;
                            self.childs.swap(i, *start + *len);
                        } else {
                            i += 1;
                        }
                    }
                    if *len == 0 {
                        // Path is death; prune
                        self.trees[node].death = true;
                        add_parents(&mut to_update, &self.trees[node].parents);
                        self.nodes -= 1;
                    } else {
                        let children = &mut self.childs[*start .. *start+*len];
                        let trees = &self.trees;
                        children.sort_by_key(|c| std::cmp::Reverse(c.evaluation(trees)));
                        let best = children[0].evaluation(trees);
                        let depth = children.iter()
                            .map(|c| trees[c.node].depth)
                            .max().unwrap() + 1;

                        let tree = &mut self.trees[node];
                        // Parents only need to be updated if our evaluation/depth changed
                        if best != tree.evaluation || depth > tree.depth {
                            tree.evaluation = best;
                            tree.depth = depth.max(tree.depth);
                            add_parents(&mut to_update, &tree.parents);
                        }
                    }
                }
                Children::Speculation(possibilities) => {
                    let mut count = 0;
                    let mut deaths = 0;
                    let mut worst = None;
                    let mut total = E::default();
                    let mut depth = 0;
                    let trees = &self.trees;
                    // The value of a speculation node is the expected value of the path. Since the
                    // probability of getting each of the possible pieces is the same, this is a
                    // simple average of the values of the best paths given each possible piece.
                    // This is made slightly more complicated by the fact that we prune paths
                    // resulting in death, but if all paths for a particular piece are death, we
                    // can't prune the speculation node, but we also don't want to give a large
                    // evaluation to paths with a high probability of resulting in death. So we
                    // count death pieces as having an evaluation 1000 worse than the worst
                    // non-death path to avoid that.
                    for (_, children) in possibilities {
                        if let Some((start, len)) = children {
                            count += 1;
                            // We may have discovered some paths result in death, so remove those
                            let mut i = *start;
                            while i < *start+*len {
                                if self.trees[self.childs[i].node].death {
                                    *len -= 1;
                                    self.childs.swap(i, *start + *len);
                                } else {
                                    i += 1;
                                }
                            }
                            if *len == 0 {
                                deaths += 1;
                            } else {
                                let children = &mut self.childs[*start .. *start+*len];
                                children.sort_by_key(|c| std::cmp::Reverse(c.evaluation(trees)));
                                let best = children[0].evaluation(&trees);
                                let d = children.iter()
                                    .map(|c| trees.get(c.node).unwrap().depth)
                                    .max().unwrap() + 1;
                                depth = d.max(depth);
                                total = total + best.clone();
                                match worst {
                                    None => worst = Some(best),
                                    Some(v) if v < best => worst = Some(best),
                                    _ => {}
                                }
                            }
                        }
                    }
                    if count == deaths {
                        // Path is death; prune
                        self.trees[node].death = true;
                        add_parents(&mut to_update, &self.trees[node].parents);
                        self.nodes -= 1;
                    } else {
                        total = total + worst.unwrap().modify_death() * deaths;
                        let evaluation = total / count;
                        let tree = self.trees.get_mut(node).unwrap();
                        // Parents only need to be updated if our evaluation/depth changed
                        if evaluation != tree.evaluation || depth > tree.depth {
                            tree.evaluation = evaluation;
                            tree.depth = depth.max(tree.depth);
                            add_parents(&mut to_update, &tree.parents);
                        }
                    }
                }
            }
        }
    }

    fn create_tree(&mut self, tree: Tree<E>) -> usize {
        let index = self.trees.len();
        self.boards.insert(tree.board.clone(), index);
        self.trees.push(tree);
        self.children.push(None);
        self.nodes += 1;
        index
    }

    fn to_simplified_board(&self, b: &Board, pieces_used: u32) -> SimplifiedBoard {
        let mut grid = ArrayVec::new();
        for y in 0..40 {
            grid.push(*b.get_row(y));
        }

        SimplifiedBoard {
            grid,
            pieces_used,
            combo: b.combo,
            back_to_back: b.b2b_bonus,
            reserve: if self.use_hold {
                b.hold_piece.unwrap_or_else(|| b.next_queue().next().unwrap())
            } else {
                Piece::I
            },
            reserve_is_hold: !self.use_hold || b.hold_piece.is_some(),
            bag: b.next_bag()
        }
    }

    fn gc(&mut self) {
        self.backbuffer_children.clear();
        self.backbuffer_childs.clear();
        self.backbuffer_trees.clear();
        self.boards.clear();
        self.next_speculation.clear();

        let mut stack = vec![(0, self.root, false)];
        // Indices in the trees/children arrays are allocated before we iterate in the loop.
        self.backbuffer_trees.push(Tree {
            parents: SmallVec::new(),
            marked: false,
            ..self.trees[self.root].clone()
        });
        self.backbuffer_children.push(None);
        self.boards.insert(self.trees[self.root].board.clone(), 0);
        self.root = 0;
        while let Some((new, orig, parent_spec)) = stack.pop() {
            // Remaining work for this node is to copy children over.
            match self.children[orig] {
                None => {}
                Some(Children::Known(start, len)) => {
                    let (start, len) = copy(
                        &mut stack,
                        &self.childs[start .. start+len],
                        new, false,
                        &mut self.boards,
                        &self.trees,
                        &mut self.backbuffer_trees,
                        &mut self.backbuffer_children,
                        &mut self.backbuffer_childs
                    );
                    self.backbuffer_children[new] = Some(Children::Known(start, len));
                }
                Some(Children::Speculation(possibilities)) => {
                    let mut c = EnumMap::new();
                    for (p, spec_children) in possibilities {
                        if let Some((start, len)) = spec_children {
                            c[p] = Some(copy(
                                &mut stack,
                                &self.childs[start .. start+len],
                                new, true,
                                &mut self.boards,
                                &self.trees,
                                &mut self.backbuffer_trees,
                                &mut self.backbuffer_children,
                                &mut self.backbuffer_childs
                            ));
                        }
                    }
                    self.backbuffer_children[new] = Some(Children::Speculation(c));
                    if !parent_spec {
                        self.next_speculation.insert(new);
                    }
                }
            }
        }

        std::mem::swap(&mut self.trees, &mut self.backbuffer_trees);
        std::mem::swap(&mut self.children, &mut self.backbuffer_children);
        std::mem::swap(&mut self.childs, &mut self.backbuffer_childs);
        self.generation += 1;
        self.nodes = self.trees.len();

        fn copy<E: Clone, R: Clone>(
            stack: &mut Vec<(usize, usize, bool)>,
            copying: &[Child<R>], new: usize, is_spec: bool,
            boards: &mut HashMap<SimplifiedBoard, usize>,
            old_trees: &[Tree<E>],
            trees: &mut Vec<Tree<E>>,
            children: &mut Vec<Option<Children>>,
            childs: &mut Vec<Child<R>>
        ) -> (usize, usize) {
            let begin = childs.len();
            for child in copying {
                use std::collections::hash_map::Entry;
                match boards.entry(old_trees[child.node].board.clone()) {
                    Entry::Occupied(entry) => {
                        // Don't create a copy of a node that's already been copied
                        let &idx = entry.get();
                        childs.push(Child {
                            node: idx,
                            ..child.clone()
                        });
                        trees[idx].parents.push(new);
                    }
                    Entry::Vacant(entry) => {
                        // Copy Tree, mark node for copying
                        let idx = trees.len();
                        trees.push(Tree {
                            parents: SmallVec::from_elem(new, 1),
                            board: old_trees[child.node].board.clone(),
                            marked: false,
                            ..old_trees[child.node].clone()
                        });
                        entry.insert(idx);
                        children.push(None);
                        childs.push(Child {
                            node: idx,
                            ..child.clone()
                        });
                        stack.push((idx, child.node, is_spec));
                    }
                }
            }
            (begin, childs.len() - begin)
        }
    }
}

fn add_parents(to_update: &mut VecDeque<usize>, parents: &[usize]) {
    for &parent in parents {
        if !to_update.contains(&parent) {
            to_update.push_back(parent);
        }
    }
}

fn pick<E: Evaluation<R>, R: Clone>(trees: &[Tree<E>], children: &[Child<R>]) -> usize {
    let minimum_evaluation = children.iter()
        .map(|c| c.evaluation(trees))
        .min().expect("no min");
    let weights = children.iter().enumerate()
        .map(|(i, c)| c.evaluation(trees).weight(&minimum_evaluation, i));
    let sampler = rand::distributions::WeightedIndex::new(weights).unwrap();
    let index = thread_rng().sample(sampler);
    children[index].node
}

impl Pieces {
    fn rebuild_board(&self, sb: &SimplifiedBoard) -> Board {
        let mut board = Board::new();
        let mut field = [[false; 10]; 40];
        for y in 0..40 {
            for x in 0..10 {
                field[y][x] = sb.grid[y] & 1<<x != 0;
            }
        }
        board.set_field(field);
        board.combo = sb.combo;
        board.b2b_bonus = sb.back_to_back;
        board.bag = sb.bag;
        if sb.reserve_is_hold {
            board.hold_piece = Some(sb.reserve);
        } else {
            board.add_next_piece(sb.reserve);
        }
        for i in (sb.pieces_used - self.pieces_used) as usize .. self.piece_queue.len() {
            board.add_next_piece(self.piece_queue[i]);
        }
        board
    }
}

impl<R: Clone> Child<R> {
    fn evaluation<E: Evaluation<R>>(&self, trees: &[Tree<E>]) -> E {
        trees.get(self.node).unwrap().evaluation.clone() + self.accumulated.clone()
    }
}

impl Children {
    fn is_dead(&self) -> bool {
        match self {
            &Children::Known(_, len) => len == 0,
            &Children::Speculation(possibilities) => {
                for (_, c) in possibilities {
                    if let Some((_, len)) = c {
                        if len != 0 {
                            return false
                        }
                    }
                }
                true
            }
        }
    }
}