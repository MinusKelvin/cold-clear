use std::collections::{ VecDeque, HashMap, HashSet };
use libtetris::{ Piece, FallingPiece, LockResult, Board };
use arrayvec::ArrayVec;
use smallvec::SmallVec;
use enumset::EnumSet;
use enum_map::EnumMap;
use rand::prelude::*;

pub struct TreeState {
    pub board: Board,
    pub root: NodeId,
    boards: HashMap<SimplifiedBoard, NodeId>,
    trees: Storage<Tree>,
    children: Storage<Option<Children>>,
    free: VecDeque<u32>,
    next_speculation: HashSet<NodeId>,
    pieces: Pieces,
    pub nodes: usize,
    use_hold: bool
}

struct Pieces {
    piece_queue: VecDeque<Piece>,
    pieces_used: u32
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(u32, u32);

struct Tree {
    board: SimplifiedBoard,
    parents: SmallVec<[NodeId; 4]>,
    evaluation: i32,
    depth: usize,
    marked: bool
}

pub struct ChildData {
    pub mv: FallingPiece,
    pub lock: LockResult,
    pub board: Board,
    pub accumulated: i32,
    pub evaluation: i32,
    pub hold: bool
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Child {
    pub mv: FallingPiece,
    pub lock: LockResult,
    pub node: NodeId,
    pub original_rank: usize,
    accumulated: i32,
    pub hold: bool
}

pub enum Children {
    Known(Vec<Child>),
    Speculation(EnumMap<Piece, Option<Vec<Child>>>)
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

impl TreeState {
    /// Requires that there is at least one next piece if `use_hold` is true.
    pub fn create(board: Board, use_hold: bool) -> Self {
        let b = board.clone();
        let mut this = TreeState {
            root: NodeId(0, 0),
            trees: Storage(vec![]),
            children: Storage(vec![]),
            free: VecDeque::new(),
            next_speculation: HashSet::new(),
            boards: HashMap::new(),
            pieces: Pieces {
                piece_queue: board.next_queue().collect(),
                pieces_used: 0
            },
            board,
            use_hold,
            nodes: 0
        };
        let sb = this.to_simplified_board(&b, if use_hold { 1 } else { 0 });
        this.root = this.create_tree(Tree {
            board: sb,
            parents: SmallVec::new(),
            evaluation: 0,
            depth: 0,
            marked: false
        });
        this
    }

    pub fn reset(&mut self, field: [[bool; 10]; 40], b2b: bool, combo: u32) {
        self.board.set_field(field);
        self.board.combo = combo;
        self.board.b2b_bonus = b2b;

        for i in 0..self.trees.0.len() {
            if self.trees.0[i].1.is_some() {
                self.free.push_back(i as u32);
                self.trees.0[i].1 = None;
                self.children.0[i].1 = None;
            }
        }
        self.boards.clear();
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
            evaluation: 0,
            depth: 0,
            marked: false
        });
    }

    /// To be called by a worker looking to expand the tree. `update_known`, `update_speculated`, or
    /// `unmark` should be called to provide the generated children. If this returns `None`, the
    /// leaf found is already being expanded by another worker, and you should try again later.
    pub fn find_and_mark_leaf(&mut self) -> Option<(NodeId, Board)> {
        if self.is_dead() {
            return None
        }
        let mut current = self.root;
        loop {
            match &self.children.get(current).unwrap() {
                None => {
                    let tree = self.trees.get_mut(current).unwrap();
                    if tree.marked {
                        return None
                    } else {
                        tree.marked = true;
                        return Some((
                            current,
                            self.pieces.rebuild_board(&tree.board)
                        ));
                    }
                },
                Some(Children::Known(c)) => current = pick(&self.trees, c),
                Some(Children::Speculation(c)) => {
                    let mut pick_from = ArrayVec::<[_; 7]>::new();
                    for (_, c) in c {
                        if let Some(c) = c {
                            if !c.is_empty() {
                                pick_from.push(c);
                            }
                        }
                    }
                    current = pick(&self.trees, pick_from.choose(&mut thread_rng()).unwrap());
                }
            }
        }
    }

    /// To be called when a worker has generated and evaluated the children of the node.
    pub fn update_known(&mut self, node: NodeId, children: Vec<ChildData>) {
        if self.trees.get(node).is_none() {
            // After a move is taken, the nodes not reachable from the new root are deleted.
            // This can happen between find_leaf and update_whatever calls, so we check that here.
            return
        }

        let children = self.build_children(node, children);
        *self.children.get_mut(node).unwrap() = Some(Children::Known(children));
        self.trees.get_mut(node).unwrap().marked = false;

        let mut v = VecDeque::new();
        v.push_back(node);
        self.update(v);
    }

    /// To be called when a worker has generated and evaluated the children of the node.
    pub fn update_speculated(
        &mut self, node: NodeId, mut children: EnumMap<Piece, Option<Vec<ChildData>>>
    ) {
        if self.trees.get(node).is_none() {
            // After a move is taken, the nodes not reachable from the new root are deleted.
            // This can happen between find_leaf and update_whatever calls, so we check that here.
            return
        }

        let speculation_piece_index = (
            self.trees.get(node).unwrap().board.pieces_used - self.pieces.pieces_used
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
            self.next_speculation.insert(node);
        }
        let mut childs = EnumMap::new();
        for (p, c) in children {
            if let Some(c) = c {
                childs[p] = Some(self.build_children(node, c));
            }
        }
        *self.children.get_mut(node).unwrap() = Some(Children::Speculation(childs));
        self.trees.get_mut(node).unwrap().marked = false;

        let mut v = VecDeque::new();
        v.push_back(node);
        self.update(v);
    }

    pub fn unmark(&mut self, node: NodeId) {
        self.trees.get_mut(node).unwrap().marked = false;
    }

    /// Adds the next piece and resolves the affected speculation nodes.
    pub fn add_next_piece(&mut self, piece: Piece) {
        self.pieces.piece_queue.push_back(piece);
        self.board.add_next_piece(piece);
        let mut next_speculation = HashSet::new();
        let mut to_update = VecDeque::new();
        let mut to_unparent = VecDeque::new();
        std::mem::swap(&mut self.next_speculation, &mut next_speculation);
        for node in next_speculation {
            let mut children = vec![];
            let childs = if let Some(c) = self.children.get_mut(node) {
                c.as_mut().expect("not a speculation node")
            } else {
                continue
            };
            if let Children::Speculation(possibilities) = childs {
                std::mem::swap(
                    &mut children,
                    possibilities[piece].as_mut().expect("invalid next piece")
                );
                for (_, c) in possibilities {
                    if let Some(c) = c {
                        for n in c {
                            to_unparent.push_back((node, n.node));
                        }
                    }
                }
            } else {
                unreachable!()
            }
            *childs = Children::Known(children);
            let children: &[Child] = match self.children.get(node).unwrap() {
                Some(Children::Known(children)) => children,
                _ => unreachable!()
            };
            for child in children {
                if self.children.get(child.node).unwrap().is_some() {
                    self.next_speculation.insert(child.node);
                }
            }
            to_update.push_back(node);
        }
        self.unparent(to_unparent);
        self.update(to_update);
    }

    /// Retrieve the current choice for best move.
    pub fn best_move(&self) -> Option<Child> {
        self.children.get(self.root)?.as_ref()
            .map(|c| if let Children::Known(children) = c {
                children.first().unwrap().clone()
            } else {
                panic!("Not enough next pieces to choose a move")
            })
    }

    /// Be sure to call `best_move` and check that it is `Some` before calling this.
    pub fn advance_move(&mut self) {
        let mut unparenting = VecDeque::new();
        unparenting.push_back((NodeId(0, 0), self.root));
        let child = match self.children.get(self.root).unwrap().as_ref().unwrap() {
            Children::Known(children) => children.first().unwrap(),
            Children::Speculation(_) => unreachable!()
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
        self.unparent(unparenting);
    }

    pub fn is_dead(&self) -> bool {
        self.children.get(self.root).is_none()
    }

    pub fn get_children(&self, node: NodeId) -> Option<Option<&Children>> {
        self.children.get(node).map(Option::as_ref)
    }

    pub fn depth(&self) -> usize {
        self.trees.get(self.root).unwrap().depth
    }

    fn build_children(&mut self, node: NodeId, mut children: Vec<ChildData>) -> Vec<Child> {
        let pieces_used = self.trees.get(node).unwrap().board.pieces_used;
        children.sort_by_key(|c| -c.evaluation);
        children.into_iter().enumerate()
            .map(|(i, data)| Child {
                mv: data.mv,
                lock: data.lock,
                hold: data.hold,
                original_rank: i,
                accumulated: data.accumulated,
                node: self.make_node(
                    self.to_simplified_board(&data.board, pieces_used+1),
                    node, data.evaluation
                )
            })
            .collect()
    }

    fn make_node(&mut self, board: SimplifiedBoard, parent: NodeId, eval: i32) -> NodeId {
        if self.boards.contains_key(&board) {
            let id = self.boards[&board];
            self.trees.get_mut(id).unwrap().parents.push(parent);
            id
        } else {
            self.create_tree(Tree {
                board,
                parents: SmallVec::from_elem(parent, 1),
                evaluation: eval,
                depth: 0,
                marked: false
            })
        }
    }

    fn update(&mut self, mut to_update: VecDeque<NodeId>) {
        while let Some(node) = to_update.pop_front() {
            match self.children.get_mut(node).unwrap().as_mut().unwrap() {
                Children::Known(children) => {
                    let trees = &self.trees;
                    // We may have discovered some paths result in death, so remove those
                    children.retain(|c| trees.get(c.node).is_some());
                    if children.is_empty() {
                        // Path is death; prune
                        let t = trees.get(node).unwrap();
                        add_parents(&mut to_update, t);
                        self.boards.remove(&t.board);
                        self.trees.0[node.0 as usize].1 = None;
                        self.children.0[node.0 as usize].1 = None;
                        self.free.push_back(node.0);
                        self.nodes -= 1;
                    } else {
                        children.sort_by_key(|c| -c.evaluation(trees));
                        let best = children[0].evaluation(trees);
                        let depth = children.iter()
                            .map(|c| trees.get(c.node).unwrap().depth)
                            .max().unwrap() + 1;
                        let tree = self.trees.get_mut(node).unwrap();
                        // Parents only need to be updated if our evaluation/depth changed
                        if best != tree.evaluation || depth > tree.depth {
                            tree.evaluation = best;
                            tree.depth = depth.max(tree.depth);
                            add_parents(&mut to_update, tree);
                        }
                    }
                }
                Children::Speculation(possibilities) => {
                    let mut count = 0;
                    let mut deaths = 0;
                    let mut worst = std::i32::MAX;
                    let mut total = 0;
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
                        if let Some(children) = children {
                            count += 1;
                            // We may have discovered some paths result in death, so remove those
                            children.retain(|c| trees.get(c.node).is_some());
                            if children.is_empty() {
                                deaths += 1;
                            } else {
                                children.sort_by_key(|c| -c.evaluation(trees));
                                let best = children[0].evaluation(&trees);
                                let d = children.iter()
                                    .map(|c| trees.get(c.node).unwrap().depth)
                                    .max().unwrap() + 1;
                                depth = d.max(depth);
                                total += best;
                                if best < worst {
                                    worst = best;
                                }
                            }
                        }
                    }
                    if count == deaths {
                        // Path is death; prune
                        let t = trees.get(node).unwrap();
                        add_parents(&mut to_update, t);
                        self.boards.remove(&t.board);
                        self.trees.0[node.0 as usize].1 = None;
                        self.children.0[node.0 as usize].1 = None;
                        self.free.push_back(node.0);
                        self.nodes -= 1;
                    } else {
                        total += (worst - 1000) * deaths;
                        let evaluation = total / count;
                        let tree = self.trees.get_mut(node).unwrap();
                        // Parents only need to be updated if our evaluation/depth changed
                        if evaluation != tree.evaluation || depth > tree.depth {
                            tree.evaluation = evaluation;
                            tree.depth = depth.max(tree.depth);
                            add_parents(&mut to_update, tree);
                        }
                    }
                }
            }
        }
    }

    fn create_tree(&mut self, tree: Tree) -> NodeId {
        self.nodes += 1;
        match self.free.pop_front() {
            Some(index) => {
                let gen = self.trees.0[index as usize].0 + 1;
                let id = NodeId(index, gen);
                self.boards.insert(tree.board.clone(), id);
                self.trees.0[index as usize] = (gen, Some(tree));
                self.children.0[index as usize] = (gen, Some(None));
                id
            }
            None => {
                let index = self.trees.0.len() as u32;
                let id = NodeId(index, 0);
                self.boards.insert(tree.board.clone(), id);
                self.trees.0.push((0, Some(tree)));
                self.children.0.push((0, Some(None)));
                id
            }
        }
    }

    fn unparent(&mut self, mut to_unparent: VecDeque<(NodeId, NodeId)>) {
        while let Some((parent, child)) = to_unparent.pop_front() {
            if let Some(c) = self.trees.get_mut(child) {
                c.parents.retain(|&mut n| n != parent);
                if c.parents.is_empty() && child != self.root {
                    // There are no remaining references to this node, so we destroy it...
                    let t = &mut self.trees.0[child.0 as usize];
                    self.boards.remove(&t.1.as_ref().unwrap().board);
                    t.1 = None;
                    let mut children = None;
                    std::mem::swap(&mut self.children.0[child.0 as usize].1, &mut children);
                    self.free.push_back(child.0);
                    self.nodes -= 1;
                    // ...requiring us to unparent its children
                    match children.unwrap() {
                        None => {}
                        Some(Children::Known(grandchildren)) => for grandchild in grandchildren {
                            to_unparent.push_back((child, grandchild.node));
                        }
                        Some(Children::Speculation(possibilities)) =>
                            for (_, gcs) in possibilities {
                                if let Some(gcs) = gcs {
                                    for grandchild in gcs {
                                        to_unparent.push_back((child, grandchild.node));
                                    }
                                }
                            }
                    }
                }
            }
        }
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
}

fn add_parents(to_update: &mut VecDeque<NodeId>, tree: &Tree) {
    for &parent in &tree.parents {
        if !to_update.contains(&parent) {
            to_update.push_back(parent);
        }
    }
}

fn pick(trees: &Storage<Tree>, children: &[Child]) -> NodeId {
    let minimum_evaluation = children.iter()
        .map(|c| trees.get(c.node).expect("child gone").evaluation)
        .min().expect("no min");
    let weights = children.iter().enumerate().map(|(i, c)| {
        let e = (trees.get(c.node).expect("child gone 2").evaluation - minimum_evaluation) as i64 + 10;
        e * e / (i + 1) as i64
    });
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

impl Child {
    fn evaluation(&self, trees: &Storage<Tree>) -> i32 {
        self.accumulated + trees.get(self.node).unwrap().evaluation
    }
}

struct Storage<T>(Vec<(u32, Option<T>)>);

impl<T> Storage<T> {
    fn get(&self, node: NodeId) -> Option<&T> {
        let (gen, tree) = self.0.get(node.0 as usize)?;
        if *gen == node.1 {
            tree.as_ref()
        } else {
            None
        }
    }

    fn get_mut(&mut self, node: NodeId) -> Option<&mut T> {
        let (gen, tree) = self.0.get_mut(node.0 as usize)?;
        if *gen == node.1 {
            tree.as_mut()
        } else {
            None
        }
    }
}