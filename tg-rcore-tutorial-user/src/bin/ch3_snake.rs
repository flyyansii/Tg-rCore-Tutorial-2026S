#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{sleep, try_getchar};

const WIDTH: i32 = 24;
const HEIGHT: i32 = 12;
const MAX_SNAKE: usize = 64;
const MAX_FRAMES: usize = 90;

#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

fn print_cell(snake: &[Point; MAX_SNAKE], len: usize, food: Point, x: i32, y: i32) {
    if food.x == x && food.y == y {
        print!("*");
        return;
    }
    for (idx, point) in snake.iter().take(len).enumerate() {
        if point.x == x && point.y == y {
            if idx == 0 {
                print!("@");
            } else {
                print!("o");
            }
            return;
        }
    }
    print!(" ");
}

fn draw(snake: &[Point; MAX_SNAKE], len: usize, food: Point, score: usize, frame: usize) {
    print!("\x1b[2J\x1b[H");
    println!("ch3 snake demo | wasd control | score {score} | frame {frame}/{MAX_FRAMES}");
    print!("+");
    for _ in 0..WIDTH {
        print!("-");
    }
    println!("+");
    for y in 0..HEIGHT {
        print!("|");
        for x in 0..WIDTH {
            print_cell(snake, len, food, x, y);
        }
        println!("|");
    }
    print!("+");
    for _ in 0..WIDTH {
        print!("-");
    }
    println!("+");
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
        if let Some(key) = try_getchar() {
            match key {
                b'w' | b'W' if dy == 0 => {
                    dx = 0;
                    dy = -1;
                }
                b's' | b'S' if dy == 0 => {
                    dx = 0;
                    dy = 1;
                }
                b'a' | b'A' if dx == 0 => {
                    dx = -1;
                    dy = 0;
                }
                b'd' | b'D' if dx == 0 => {
                    dx = 1;
                    dy = 0;
                }
                _ => {}
            }
        }

        let mut head = Point::new(
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
        sleep(80);
        head = snake[0];
        if snake[1..len].iter().any(|p| p.x == head.x && p.y == head.y) {
            println!("snake touched itself, restarting shape");
            len = 4;
        }
    }

    println!("Test ch3 snake OK!");
    0
}
