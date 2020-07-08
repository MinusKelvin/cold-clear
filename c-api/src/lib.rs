use std::mem::MaybeUninit;
use enumset::EnumSet;

type CCAsyncBot = cold_clear::Interface;

macro_rules! cenum {
    ($($(#[$a:meta])* enum $name:ident => $t:ty { $($item:ident => $to:ident),* })*) => {
        $(
        $(#[$a])*
        #[repr(C)]
        #[derive(Copy, Clone, Debug)]
        #[allow(non_camel_case_types)]
        enum $name {
            $($item),*
        }

        impl From<$name> for $t {
            fn from(v: $name) -> $t {
                #[allow(unreachable_patterns)]
                match v {
                    $(
                        $name::$item => <$t>::$to,
                    )*
                    _ => unreachable!()
                }
            }
        }

        impl From<$t> for $name {
            fn from(v: $t) -> $name {
                #[allow(unreachable_patterns)]
                match v {
                    $(
                        <$t>::$to => $name::$item,
                    )*
                    _ => unreachable!()
                }
            }
        }
        )*
    };
}

cenum! {
    enum CCPiece => libtetris::Piece {
        CC_I => I,
        CC_O => O,
        CC_T => T,
        CC_L => L,
        CC_J => J,
        CC_S => S,
        CC_Z => Z
    }

    enum CCTspinStatus => libtetris::TspinStatus {
        CC_NONE => None,
        CC_MINI => Mini,
        CC_FULL => Full
    }

    enum CCMovement => libtetris::PieceMovement {
        CC_LEFT => Left,
        CC_RIGHT => Right,
        CC_CW => Cw,
        CC_CCW => Ccw,
        CC_DROP => SonicDrop
    }

    enum CCSpawnRule => libtetris::SpawnRule {
        CC_ROW_19_OR_20 => Row19Or20,
        CC_ROW_21_AND_FALL => Row21AndFall
    }

    enum CCMovementMode => cold_clear::moves::MovementMode {
        CC_0G => ZeroG,
        CC_20G => TwentyG,
        CC_HARD_DROP_ONLY => HardDropOnly
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[allow(non_camel_case_types)]
enum CCBotPollStatus {
    CC_MOVE_PROVIDED,
    CC_WAITING,
    CC_BOT_DEAD
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct CCMove {
    hold: bool,
    expected_x: [u8; 4],
    expected_y: [u8; 4],
    movement_count: u8,
    movements: [CCMovement; 32],
    nodes: u32,
    depth: u32,
    original_rank: u32,
    evaluation_result: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct CCPlanPlacement {
    piece: CCPiece,
    tspin: CCTspinStatus,
    expected_x: [u8; 4],
    expected_y: [u8; 4],
    cleared_lines: [i32; 4],
}

#[repr(C)]
struct CCOptions {
    mode: CCMovementMode,
    spawn_rule: CCSpawnRule,
    use_hold: bool,
    speculate: bool,
    pcloop: bool,
    min_nodes: u32,
    max_nodes: u32,
    threads: u32,
}

#[repr(C)]
struct CCWeights {
    back_to_back: i32,
    bumpiness: i32,
    bumpiness_sq: i32,
    height: i32,
    top_half: i32,
    top_quarter: i32,
    jeopardy: i32,
    cavity_cells: i32,
    cavity_cells_sq: i32,
    overhang_cells: i32,
    overhang_cells_sq: i32,
    covered_cells: i32,
    covered_cells_sq: i32,
    tslot: [i32; 4],
    well_depth: i32,
    max_well_depth: i32,
    well_column: [i32; 10],

    b2b_clear: i32,
    clear1: i32,
    clear2: i32,
    clear3: i32,
    clear4: i32,
    tspin1: i32,
    tspin2: i32,
    tspin3: i32,
    mini_tspin1: i32,
    mini_tspin2: i32,
    perfect_clear: i32,
    combo_garbage: i32,
    move_time: i32,
    wasted_t: i32,

    use_bag: bool,
}

fn convert_hold(hold: *mut CCPiece) -> Option<libtetris::Piece> {
    if hold.is_null() {
        None
    } else {
        Some(unsafe{*hold}.into())
    }
}


fn convert_from_c_options(options: &CCOptions) -> cold_clear::Options {
    cold_clear::Options {
        max_nodes: options.max_nodes,
        min_nodes: options.min_nodes,
        use_hold: options.use_hold,
        speculate: options.speculate,
        pcloop: options.pcloop,
        mode: options.mode.into(),
        spawn_rule: options.spawn_rule.into(),
        threads: options.threads
    }
}

fn convert_from_c_weights(weights: &CCWeights) -> cold_clear::evaluation::Standard {
    cold_clear::evaluation::Standard {
        back_to_back: weights.back_to_back,
        bumpiness: weights.bumpiness,
        bumpiness_sq: weights.bumpiness_sq,
        height: weights.height,
        top_half: weights.top_half,
        top_quarter: weights.top_quarter,
        jeopardy: weights.jeopardy,
        cavity_cells: weights.cavity_cells,
        cavity_cells_sq: weights.cavity_cells_sq,
        overhang_cells: weights.overhang_cells,
        overhang_cells_sq: weights.overhang_cells_sq,
        covered_cells: weights.covered_cells,
        covered_cells_sq: weights.covered_cells_sq,
        tslot: weights.tslot,
        well_depth: weights.well_depth,
        max_well_depth: weights.max_well_depth,
        well_column: weights.well_column,

        b2b_clear: weights.b2b_clear,
        clear1: weights.clear1,
        clear2: weights.clear2,
        clear3: weights.clear3,
        clear4: weights.clear4,
        tspin1: weights.tspin1,
        tspin2: weights.tspin2,
        tspin3: weights.tspin3,
        mini_tspin1: weights.mini_tspin1,
        mini_tspin2: weights.mini_tspin2,
        perfect_clear: weights.perfect_clear,
        combo_garbage: weights.combo_garbage,
        move_time: weights.move_time,
        wasted_t: weights.wasted_t,

        use_bag: weights.use_bag,
        sub_name: None
    }
}

#[no_mangle]
extern "C" fn cc_launch_with_board_async(options: &CCOptions, weights: &CCWeights, field: &[[bool; 10]; 40], 
    bag_remain: u32, hold: *mut CCPiece, b2b: bool, combo: u32) -> *mut CCAsyncBot {
    Box::into_raw(Box::new(cold_clear::Interface::launch(
        libtetris::Board::new_with_state(*field, EnumSet::from_bits(bag_remain as u128), convert_hold(hold), b2b, combo),
        convert_from_c_options(options),
        convert_from_c_weights(weights)
    )))
}

#[no_mangle]
extern "C" fn cc_launch_async(options: &CCOptions, weights: &CCWeights) -> *mut CCAsyncBot {
    Box::into_raw(Box::new(cold_clear::Interface::launch(
        libtetris::Board::new(),
        convert_from_c_options(options),
        convert_from_c_weights(weights)
    )))
}

#[no_mangle]
extern "C" fn cc_destroy_async(bot: *mut CCAsyncBot) {
    unsafe { Box::from_raw(bot); }
}

#[no_mangle]
extern "C" fn cc_reset_async(
    bot: &mut CCAsyncBot, field: &[[bool; 10]; 40], b2b: bool, combo: u32
) {
    bot.reset(*field, b2b, combo);
}

#[no_mangle]
extern "C" fn cc_add_next_piece_async(bot: &mut CCAsyncBot, piece: CCPiece) {
    bot.add_next_piece(piece.into());
}

#[no_mangle]
extern "C" fn cc_request_next_move(bot: &mut CCAsyncBot, incoming: u32) {
    bot.request_next_move(incoming);
}

fn convert_plan_placement(
    (falling_piece, lock_result): &(libtetris::FallingPiece, libtetris::LockResult)
) -> CCPlanPlacement {
    let mut expected_x = [0; 4];
    let mut expected_y = [0; 4];
    for (i, &(x, y)) in falling_piece.cells().iter().enumerate() {
        expected_x[i] = x as u8;
        expected_y[i] = y as u8;
    }

    let mut cleared_lines = [-1; 4];
    for (i, &cl) in lock_result.cleared_lines.iter().enumerate() {
        cleared_lines[i] = cl;
    }

    CCPlanPlacement {
        piece: falling_piece.kind.0.into(),
        tspin: falling_piece.tspin.into(),
        expected_x: expected_x,
        expected_y: expected_y,
        cleared_lines: cleared_lines,
    }
}

fn convert_plan(
    info: &cold_clear::Info,
    plan: *mut MaybeUninit<CCPlanPlacement>,
    plan_length: *mut u32
) {
    if !plan.is_null() && !plan_length.is_null() {
        let plan_length = unsafe { &mut *plan_length };
        let plan = unsafe {
            std::slice::from_raw_parts_mut(plan, *plan_length as usize)
        };
        let n = info.plan.len().min(plan.len());
        for i in 0..n {
            plan[i] = MaybeUninit::new(convert_plan_placement(&info.plan[i]));
        }
        *plan_length = n as u32;
    }
}

fn convert(m: cold_clear::Move, info: cold_clear::Info) -> CCMove {
    let mut expected_x = [0; 4];
    let mut expected_y = [0; 4];
    for (i, &(x, y)) in m.expected_location.cells().iter().enumerate() {
        expected_x[i] = x as u8;
        expected_y[i] = y as u8;
    }
    let mut movements = [CCMovement::CC_DROP; 32];
    for (i, &mv) in m.inputs.iter().enumerate() {
        movements[i] = mv.into();
    }
    CCMove {
        hold: m.hold,
        expected_x,
        expected_y,
        movement_count: m.inputs.len() as u8,
        movements,
        nodes: info.nodes as u32,
        depth: info.depth as u32,
        original_rank: info.original_rank as u32,
        evaluation_result: info.evaluation_result,
    }
}

#[no_mangle]
extern "C" fn cc_poll_next_move(
    bot: &mut CCAsyncBot,
    mv: *mut CCMove,
    plan: *mut MaybeUninit<CCPlanPlacement>,
    plan_length: *mut u32
) -> CCBotPollStatus {
    match bot.poll_next_move() {
        Ok((m, info)) => {
            convert_plan(&info, plan, plan_length);
            unsafe { mv.write(convert(m, info)) };
            CCBotPollStatus::CC_MOVE_PROVIDED
        }
        Err(cold_clear::BotPollState::Waiting) => CCBotPollStatus::CC_WAITING,
        Err(cold_clear::BotPollState::Dead) => CCBotPollStatus::CC_BOT_DEAD,
    }
}

#[no_mangle]
extern "C" fn cc_block_next_move(
    bot: &mut CCAsyncBot,
    mv: *mut CCMove,
    plan: *mut MaybeUninit<CCPlanPlacement>,
    plan_length: *mut u32
) -> CCBotPollStatus {
    match bot.block_next_move() {
        Some((m, info)) => {
            convert_plan(&info, plan, plan_length);
            unsafe { mv.write(convert(m, info)) };
            CCBotPollStatus::CC_MOVE_PROVIDED
        }
        None => CCBotPollStatus::CC_BOT_DEAD,
    }
}

#[no_mangle]
unsafe extern "C" fn cc_default_options(options: *mut CCOptions) {
    let o = cold_clear::Options::default();
    options.write(CCOptions {
        max_nodes: o.max_nodes,
        min_nodes: o.min_nodes,
        use_hold: o.use_hold,
        speculate: o.speculate,
        pcloop: o.pcloop,
        mode: o.mode.into(),
        spawn_rule: o.spawn_rule.into(),
        threads: o.threads
    });
}

fn convert_weights(w: cold_clear::evaluation::Standard) -> CCWeights {
    CCWeights {
        back_to_back: w.back_to_back,
        bumpiness: w.bumpiness,
        bumpiness_sq: w.bumpiness_sq,
        height: w.height,
        top_half: w.top_half,
        top_quarter: w.top_quarter,
        jeopardy: w.jeopardy,
        cavity_cells: w.cavity_cells,
        cavity_cells_sq: w.cavity_cells_sq,
        overhang_cells: w.overhang_cells,
        overhang_cells_sq: w.overhang_cells_sq,
        covered_cells: w.covered_cells,
        covered_cells_sq: w.covered_cells_sq,
        tslot: w.tslot,
        well_depth: w.well_depth,
        max_well_depth: w.max_well_depth,
        well_column: w.well_column,

        b2b_clear: w.b2b_clear,
        clear1: w.clear1,
        clear2: w.clear2,
        clear3: w.clear3,
        clear4: w.clear4,
        tspin1: w.tspin1,
        tspin2: w.tspin2,
        tspin3: w.tspin3,
        mini_tspin1: w.mini_tspin1,
        mini_tspin2: w.mini_tspin2,
        perfect_clear: w.perfect_clear,
        combo_garbage: w.combo_garbage,
        move_time: w.move_time,
        wasted_t: w.wasted_t,

        use_bag: w.use_bag
    }
}

#[no_mangle]
unsafe extern "C" fn cc_default_weights(weights: *mut CCWeights) {
    weights.write(convert_weights(cold_clear::evaluation::Standard::default()));
}

#[no_mangle]
unsafe extern "C" fn cc_fast_weights(weights: *mut CCWeights) {
    weights.write(convert_weights(cold_clear::evaluation::Standard::fast_config()));
}