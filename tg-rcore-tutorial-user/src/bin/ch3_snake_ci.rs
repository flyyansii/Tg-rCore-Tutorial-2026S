#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{sleep, write};

const WIDTH: i32 = 24;
const HEIGHT: i32 = 12;
const MAX_SNAKE: usize = 64;
const MAX_FRAMES: usize = 90;
const GRAPHICS_FD: usize = 3;
const SNAKE_FRAME_MAGIC: u32 = 0x534E_4B33;

#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FramePoint {
    x: u8,
    y: u8,
}

impl FramePoint {
    const fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

#[repr(C)]
struct SnakeFrame {
    magic: u32,
    width: u32,
    height: u32,
    len: u32,
    score: u32,
    food: FramePoint,
    _padding: [u8; 2],
    snake: [FramePoint; MAX_SNAKE],
}

impl Point {
    const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

fn draw(snake: &[Point; MAX_SNAKE], len: usize, food: Point, score: usize, frame: usize) {
    if frame % 15 == 0 {
        println!("ch3 snake CI frame {frame}/{MAX_FRAMES}, score {score}");
    }
    let mut frame_buf = SnakeFrame {
        magic: SNAKE_FRAME_MAGIC,
        width: WIDTH as u32,
        height: HEIGHT as u32,
        len: len as u32,
        score: score as u32,
        food: FramePoint {
            x: food.x as u8,
            y: food.y as u8,
        },
        _padding: [0; 2],
        snake: [FramePoint::zero(); MAX_SNAKE],
    };
    let mut i = 0;
    while i < len {
        frame_buf.snake[i] = FramePoint {
            x: snake[i].x as u8,
            y: snake[i].y as u8,
        };
        i += 1;
    }
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &frame_buf as *const SnakeFrame as *const u8,
            core::mem::size_of::<SnakeFrame>(),
        )
    };
    write(GRAPHICS_FD, bytes);
}

fn next_food(food: Point) -> Point {
    Point::new(
        (food.x * 7 + 5).rem_euclid(WIDTH),
        (food.y * 5 + 3).rem_euclid(HEIGHT),
    )
}

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    let mut snake = [Point::new(0, 0); MAX_SNAKE];
    let mut len = 4usize;
    snake[0] = Point::new(6, 4);
    snake[1] = Point::new(5, 4);
    snake[2] = Point::new(4, 4);
    snake[3] = Point::new(3, 4);

    let mut dx = 1;
    let mut dy = 0;
    let mut food = Point::new(14, 6);
    let mut score = 0usize;

    for frame in 0..=MAX_FRAMES {
        if frame == 20 {
            dx = 0;
            dy = 1;
        } else if frame == 45 {
            dx = 1;
            dy = 0;
        } else if frame == 70 {
            dx = 0;
            dy = -1;
        }

        let head = Point::new(
            (snake[0].x + dx).rem_euclid(WIDTH),
            (snake[0].y + dy).rem_euclid(HEIGHT),
        );
        let ate = head.x == food.x && head.y == food.y;
        if ate && len < MAX_SNAKE {
            len += 1;
            score += 1;
            food = next_food(food);
        }

        for i in (1..len).rev() {
            snake[i] = snake[i - 1];
        }
        snake[0] = head;

        draw(&snake, len, food, score, frame);
        sleep(40);
    }

    println!("Test ch3 snake OK!");
    0
}
