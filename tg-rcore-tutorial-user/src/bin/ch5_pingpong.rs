#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{get_time, sched_yield, sleep, try_getchar, write};

const WIDTH: i32 = 680;
const HEIGHT: i32 = 390;
const PADDLE_H: i32 = 78;
const PADDLE_SPEED: i32 = 18;
const BALL_SIZE: i32 = 14;
const GRAPHICS_FD: usize = 3;
const PINGPONG_FRAME_MAGIC: u32 = 0x504F_4E47;

#[repr(C)]
struct PingpongFrame {
    magic: u32,
    width: u32,
    height: u32,
    left_y: i32,
    right_y: i32,
    ball_x: i32,
    ball_y: i32,
    left_score: u32,
    right_score: u32,
    speed: u32,
    game_over: u32,
}

fn submit_frame(
    left_y: i32,
    right_y: i32,
    ball_x: i32,
    ball_y: i32,
    left_score: usize,
    right_score: usize,
    speed: usize,
    game_over: bool,
) {
    let frame = PingpongFrame {
        magic: PINGPONG_FRAME_MAGIC,
        width: WIDTH as u32,
        height: HEIGHT as u32,
        left_y,
        right_y,
        ball_x,
        ball_y,
        left_score: left_score as u32,
        right_score: right_score as u32,
        speed: speed as u32,
        game_over: game_over as u32,
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &frame as *const PingpongFrame as *const u8,
            core::mem::size_of::<PingpongFrame>(),
        )
    };
    write(GRAPHICS_FD, bytes);
}

fn clamp_paddle(y: i32) -> i32 {
    y.max(0).min(HEIGHT - PADDLE_H)
}

fn reset_ball(left_score: usize, right_score: usize) -> (i32, i32, i32, i32) {
    let dir = if (left_score + right_score) & 1 == 0 { 1 } else { -1 };
    (WIDTH / 2, HEIGHT / 2, 7 * dir, 5)
}

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    let mut left_y = HEIGHT / 2 - PADDLE_H / 2;
    let mut right_y = left_y;
    let mut left_score = 0usize;
    let mut right_score = 0usize;
    let mut speed = 1usize;
    let (mut ball_x, mut ball_y, mut vx, mut vy) = reset_ball(left_score, right_score);
    let mut last_tick = get_time();

    println!("ch5 pingpong: left w/s, right i/k, q quits");
    submit_frame(left_y, right_y, ball_x, ball_y, left_score, right_score, speed, false);

    loop {
        while let Some(key) = try_getchar() {
            match key {
                b'w' | b'W' => left_y = clamp_paddle(left_y - PADDLE_SPEED),
                b's' | b'S' => left_y = clamp_paddle(left_y + PADDLE_SPEED),
                b'i' | b'I' => right_y = clamp_paddle(right_y - PADDLE_SPEED),
                b'k' | b'K' => right_y = clamp_paddle(right_y + PADDLE_SPEED),
                b'q' | b'Q' => {
                    submit_frame(
                        left_y,
                        right_y,
                        ball_x,
                        ball_y,
                        left_score,
                        right_score,
                        speed,
                        true,
                    );
                    println!("Test ch5 pingpong OK!");
                    return 0;
                }
                _ => {}
            }
        }

        let now = get_time();
        if now - last_tick >= 24 {
            last_tick = now;
            ball_x += vx;
            ball_y += vy;

            if ball_y <= 0 || ball_y + BALL_SIZE >= HEIGHT {
                vy = -vy;
                ball_y = ball_y.max(0).min(HEIGHT - BALL_SIZE);
            }

            let left_hit = ball_x <= 34
                && ball_x >= 20
                && ball_y + BALL_SIZE >= left_y
                && ball_y <= left_y + PADDLE_H;
            let right_hit = ball_x + BALL_SIZE >= WIDTH - 34
                && ball_x + BALL_SIZE <= WIDTH - 20
                && ball_y + BALL_SIZE >= right_y
                && ball_y <= right_y + PADDLE_H;
            if left_hit {
                vx = vx.abs() + if speed < 10 { 1 } else { 0 };
                speed = (speed + 1).min(99);
            } else if right_hit {
                vx = -vx.abs() - if speed < 10 { 1 } else { 0 };
                speed = (speed + 1).min(99);
            }

            if ball_x < -BALL_SIZE {
                right_score += 1;
                speed = 1;
                (ball_x, ball_y, vx, vy) = reset_ball(left_score, right_score);
            } else if ball_x > WIDTH {
                left_score += 1;
                speed = 1;
                (ball_x, ball_y, vx, vy) = reset_ball(left_score, right_score);
            }

            submit_frame(left_y, right_y, ball_x, ball_y, left_score, right_score, speed, false);
        }

        sleep(6);
        sched_yield();
    }
}
