#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

typedef struct CCAsyncBot CCAsyncBot;

typedef enum CCPiece {
    CC_I, CC_O, CC_T, CC_L, CC_J, CC_S, CC_Z
} CCPiece;

typedef enum CCTspinStatus {
    CC_NONE_TSPIN_STATUS,
    CC_MINI,
    CC_FULL,
} CCTspinStatus;

typedef enum CCMovement {
    CC_LEFT, CC_RIGHT,
    CC_CW, CC_CCW,
    /* Soft drop all the way down */
    CC_DROP
} CCMovement;

typedef enum CCMovementMode {
    CC_0G,
    CC_20G,
    CC_HARD_DROP_ONLY
} CCMovementMode;

typedef enum CCSpawnRule {
    CC_ROW_19_OR_20,
    CC_ROW_21_AND_FALL,
} CCSpawnRule;

typedef enum CCBotPollStatus {
    CC_MOVE_PROVIDED,
    CC_WAITING,
    CC_BOT_DEAD
} CCBotPollStatus;

typedef enum CCPcPriority {
    CC_PC_OFF,
    CC_PC_FASTEST,
    CC_PC_ATTACK
} CCPcPriority;

typedef struct CCPlanPlacement {
    CCPiece piece;
    CCTspinStatus tspin;

    /* Expected cell coordinates of placement, (0, 0) being the bottom left */
    uint8_t expected_x[4];
    uint8_t expected_y[4];

    /* Expected lines that will be cleared after placement, with -1 indicating no line */
    int32_t cleared_lines[4];
} CCPlanPlacement;

typedef struct CCMove {
    /* Whether hold is required */
    bool hold;
    /* Expected cell coordinates of placement, (0, 0) being the bottom left */
    uint8_t expected_x[4];
    uint8_t expected_y[4];
    /* Number of moves in the path */
    uint8_t movement_count;
    /* Movements */
    CCMovement movements[32];

    /* Bot Info */
    uint32_t nodes;
    uint32_t depth;
    uint32_t original_rank;
} CCMove;

typedef struct CCOptions {
    CCMovementMode mode;
    CCSpawnRule spawn_rule;
    CCPcPriority pcloop;
    uint32_t min_nodes;
    uint32_t max_nodes;
    uint32_t threads;
    bool use_hold;
    bool speculate;
} CCOptions;

typedef struct CCWeights {
    int32_t back_to_back;
    int32_t bumpiness;
    int32_t bumpiness_sq;
    int32_t row_transitions;
    int32_t height;
    int32_t top_half;
    int32_t top_quarter;
    int32_t jeopardy;
    int32_t cavity_cells;
    int32_t cavity_cells_sq;
    int32_t overhang_cells;
    int32_t overhang_cells_sq;
    int32_t covered_cells;
    int32_t covered_cells_sq;
    int32_t tslot[4];
    int32_t well_depth;
    int32_t max_well_depth;
    int32_t well_column[10];

    int32_t b2b_clear;
    int32_t clear1;
    int32_t clear2;
    int32_t clear3;
    int32_t clear4;
    int32_t tspin1;
    int32_t tspin2;
    int32_t tspin3;
    int32_t mini_tspin1;
    int32_t mini_tspin2;
    int32_t perfect_clear;
    int32_t combo_garbage;
    int32_t move_time;
    int32_t wasted_t;

    bool use_bag;
    bool timed_jeopardy;
    bool stack_pc_damage;
} CCWeights;

/* Launches a bot thread with a blank board, empty queue, and all seven pieces in the bag, using the
 * specified options and weights.
 *
 * You pass the returned pointer with `cc_destroy_async` when you are done with the bot instance.
 * 
 * Lifetime: The returned pointer is valid until it is passed to `cc_destroy_async`.
 */
CCAsyncBot *cc_launch_async(CCOptions *options, CCWeights *weights);

/* Launches a bot thread with a predefined field, empty queue, remaining pieces in the bag, hold piece,
 * back-to-back status, and combo count. This allows you to start CC from the middle of a game.
 * 
 * The bag_remain parameter is a bit field indicating which pieces are still in the bag. Each bit
 * correspond to CCPiece enum. This must match the next few pieces provided to CC via
 * cc_add_next_piece_async later.
 * 
 * The field parameter is a pointer to the start of an array of 400 booleans in row major order,
 * with index 0 being the bottom-left cell.
 * 
 * The hold parameter is a pointer to the current hold piece, or `NULL` if there's no hold piece now.
 */
CCAsyncBot *cc_launch_with_board_async(CCOptions *options, CCWeights *weights, bool *field,
    uint32_t bag_remain, CCPiece *hold, bool b2b, uint32_t combo);

/* Terminates the bot thread and frees the memory associated with the bot.
 */
void cc_destroy_async(CCAsyncBot *bot);

/* Resets the playfield, back-to-back status, and combo count.
 * 
 * This should only be used when garbage is received or when your client could not place the
 * piece in the correct position for some reason (e.g. 15 move rule), since this forces the
 * bot to throw away previous computations.
 * 
 * Note: combo is not the same as the displayed combo in guideline games. Here, it is the
 * number of consecutive line clears achieved. So, generally speaking, if "x Combo" appears
 * on the screen, you need to use x+1 here.
 * 
 * The field parameter is a pointer to the start of an array of 400 booleans in row major order,
 * with index 0 being the bottom-left cell.
 */
void cc_reset_async(CCAsyncBot *bot, bool *field, bool b2b, uint32_t combo);

/* Adds a new piece to the end of the queue.
 * 
 * If speculation is enabled, the piece must be in the bag. For example, if you start a new
 * game with starting sequence IJOZT, the first time you call this function you can only
 * provide either an L or an S piece.
 */
void cc_add_next_piece_async(CCAsyncBot *bot, CCPiece piece);

/* Request the bot to provide a move as soon as possible.
 * 
 * In most cases, "as soon as possible" is a very short amount of time, and is only longer if
 * the provided lower limit on thinking has not been reached yet or if the bot cannot provide
 * a move yet, usually because it lacks information on the next pieces.
 * 
 * For example, in a game with zero piece previews and hold enabled, the bot will never be able
 * to provide the first move because it cannot know what piece it will be placing if it chooses
 * to hold. Another example: in a game with zero piece previews and hold disabled, the bot
 * will only be able to provide a move after the current piece spawns and you provide the piece
 * information to the bot using `cc_add_next_piece_async`.
 * 
 * It is recommended that you call this function the frame before the piece spawns so that the
 * bot has time to finish its current thinking cycle and supply the move.
 * 
 * Once a move is chosen, the bot will update its internal state to the result of the piece
 * being placed correctly and the move will become available by calling `cc_poll_next_move`.
 * 
 * The incoming parameter specifies the number of lines of garbage the bot is expected to receive
 * after placing the next piece.
 */
void cc_request_next_move(CCAsyncBot *bot, uint32_t incoming);

/* Checks to see if the bot has provided the previously requested move yet.
 * 
 * The returned move contains both a path and the expected location of the placed piece. The
 * returned path is reasonably good, but you might want to use your own pathfinder to, for
 * example, exploit movement intricacies in the game you're playing.
 * 
 * If the piece couldn't be placed in the expected location, you must call `cc_reset_async` to
 * reset the game field, back-to-back status, and combo values.
 * 
 * If `plan` and `plan_length` are not `NULL` and this function provides a move, a placement plan
 * will be returned in the array pointed to by `plan`. `plan_length` should point to the length
 * of the array, and the number of plan placements provided will be returned through this pointer.
 * 
 * If the move has been provided, this function will return `CC_MOVE_PROVIDED`.
 * If the bot has not produced a result, this function will return `CC_WAITING`.
 * If the bot has found that it cannot survive, this function will return `CC_BOT_DEAD`
 */
CCBotPollStatus cc_poll_next_move(
    CCAsyncBot *bot,
    CCMove *move,
    CCPlanPlacement* plan,
    uint32_t *plan_length
);

/* This function is the same as `cc_poll_next_move` except when `cc_poll_next_move` would return
 * `CC_WAITING` it instead waits until the bot has made a decision.
 *
 * If the move has been provided, this function will return `CC_MOVE_PROVIDED`.
 * If the bot has found that it cannot survive, this function will return `CC_BOT_DEAD`
 */
CCBotPollStatus cc_block_next_move(
    CCAsyncBot *bot,
    CCMove *move,
    CCPlanPlacement* plan,
    uint32_t *plan_length
);

/* Returns the default options in the options parameter */
void cc_default_options(CCOptions *options);

/* Returns the default weights in the weights parameter */
void cc_default_weights(CCWeights *weights);

/* Returns the fast game config weights in the weights parameter */
void cc_fast_weights(CCWeights *weights);
