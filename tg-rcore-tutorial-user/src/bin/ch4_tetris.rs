#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{get_time, sched_yield, sleep, try_getchar, write};

const WIDTH: usize = 10;
const HEIGHT: usize = 20;
const TICK_MIN_MS: isize = 90;
const TICK_START_MS: isize = 520;
const GRAPHICS_FD: usize = 3;
const TETRIS_FRAME_MAGIC: u32 = 0x5454_5234;

#[derive(Clone, Copy)]
struct Piece {
    kind: usize,
    rot: usize,
    x: i32,
    y: i32,
}

#[repr(C)]
struct TetrisFrame {
    magic: u32,
    width: u32,
    height: u32,
    score: u32,
    lines: u32,
    level: u32,
    game_over: u32,
    cells: [u8; WIDTH * HEIGHT],
}

const SHAPES: [[[(i32, i32); 4]; 4]; 7] = [
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
];

const GLYPHS: [u8; 7] = [b'I', b'O', b'T', b'S', b'Z', b'J', b'L'];

fn new_piece(index: usize) -> Piece {
    Piece {
        kind: (index * 5 + 3) % 7,
        rot: 0,
        x: 3,
        y: 0,
    }
}

fn occupied(piece: Piece, i: usize) -> (i32, i32) {
    let (x, y) = SHAPES[piece.kind][piece.rot][i];
    (piece.x + x, piece.y + y)
}

fn collides(board: &[[u8; WIDTH]; HEIGHT], piece: Piece) -> bool {
    let mut i = 0;
    while i < 4 {
        let (x, y) = occupied(piece, i);
        if x < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return true;
        }
        if y >= 0 && board[y as usize][x as usize] != 0 {
            return true;
        }
        i += 1;
    }
    false
}

fn place(board: &mut [[u8; WIDTH]; HEIGHT], piece: Piece) {
    let mut i = 0;
    while i < 4 {
        let (x, y) = occupied(piece, i);
        if y >= 0 && y < HEIGHT as i32 && x >= 0 && x < WIDTH as i32 {
            board[y as usize][x as usize] = GLYPHS[piece.kind];
        }
        i += 1;
    }
}

fn clear_lines(board: &mut [[u8; WIDTH]; HEIGHT]) -> usize {
    let mut cleared = 0usize;
    let mut y = HEIGHT;
    while y > 0 {
        y -= 1;
        let mut full = true;
        let mut x = 0;
        while x < WIDTH {
            if board[y][x] == 0 {
                full = false;
                break;
            }
            x += 1;
        }
        if full {
            let mut row = y;
            while row > 0 {
                board[row] = board[row - 1];
                row -= 1;
            }
            board[0] = [0; WIDTH];
            cleared += 1;
            y += 1;
        }
    }
    cleared
}

fn cell_at(board: &[[u8; WIDTH]; HEIGHT], piece: Piece, x: usize, y: usize) -> u8 {
    let mut i = 0;
    while i < 4 {
        let (px, py) = occupied(piece, i);
        if px == x as i32 && py == y as i32 {
            return GLYPHS[piece.kind];
        }
        i += 1;
    }
    board[y][x]
}

fn submit_frame(
    board: &[[u8; WIDTH]; HEIGHT],
    piece: Piece,
    score: usize,
    lines: usize,
    level: usize,
    game_over: bool,
) {
    let mut frame = TetrisFrame {
        magic: TETRIS_FRAME_MAGIC,
        width: WIDTH as u32,
        height: HEIGHT as u32,
        score: score as u32,
        lines: lines as u32,
        level: level as u32,
        game_over: game_over as u32,
        cells: [0; WIDTH * HEIGHT],
    };
    let mut y = 0;
    while y < HEIGHT {
        let mut x = 0;
        while x < WIDTH {
            frame.cells[y * WIDTH + x] = cell_at(board, piece, x, y);
            x += 1;
        }
        y += 1;
    }
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &frame as *const TetrisFrame as *const u8,
            core::mem::size_of::<TetrisFrame>(),
        )
    };
    write(GRAPHICS_FD, bytes);
}

fn handle_key(board: &[[u8; WIDTH]; HEIGHT], piece: &mut Piece, key: u8) -> bool {
    let mut next = *piece;
    match key {
        b'a' | b'A' => next.x -= 1,
        b'd' | b'D' => next.x += 1,
        b's' | b'S' => next.y += 1,
        b'w' | b'W' => next.rot = (next.rot + 1) % 4,
        b' ' => {
            while !collides(board, next) {
                *piece = next;
                next.y += 1;
            }
            return true;
        }
        b'q' | b'Q' => return false,
        _ => return true,
    }
    if !collides(board, next) {
        *piece = next;
    }
    true
}

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    let mut board = [[0u8; WIDTH]; HEIGHT];
    let mut next_index = 0usize;
    let mut piece = new_piece(next_index);
    next_index += 1;
    let mut score = 0usize;
    let mut lines = 0usize;
    let mut level = 1usize;
    let mut last_tick = get_time();

    println!("ch4 tetris: click the QEMU window, use a/d/w/s/space, q quits");
    submit_frame(&board, piece, score, lines, level, false);

    loop {
        while let Some(key) = try_getchar() {
            if !handle_key(&board, &mut piece, key) {
                println!("Test ch4 tetris OK!");
                return 0;
            }
        }

        let delay = (TICK_START_MS - (level as isize - 1) * 45).max(TICK_MIN_MS);
        let now = get_time();
        if now - last_tick >= delay {
            last_tick = now;
            let mut dropped = piece;
            dropped.y += 1;
            if collides(&board, dropped) {
                place(&mut board, piece);
                let just_cleared = clear_lines(&mut board);
                if just_cleared > 0 {
                    lines += just_cleared;
                    score += match just_cleared {
                        1 => 100,
                        2 => 300,
                        3 => 500,
                        _ => 800,
                    } * level;
                    level = 1 + lines / 5;
                } else {
                    score += 5;
                }
                piece = new_piece(next_index);
                next_index += 1;
                if collides(&board, piece) {
                    submit_frame(&board, piece, score, lines, level, true);
                    println!("Game over. Test ch4 tetris OK!");
                    return 0;
                }
            } else {
                piece = dropped;
            }
            submit_frame(&board, piece, score, lines, level, false);
        }

        sleep(15);
        sched_yield();
    }
}
