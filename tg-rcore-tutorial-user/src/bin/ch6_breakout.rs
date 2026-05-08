#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{close, get_time, open, read, sched_yield, sleep, try_getchar, write, OpenFlags};

const WIDTH: i32 = 536;
const HEIGHT: i32 = 288;
const BRICK_COLS: usize = 10;
const BRICK_ROWS: usize = 6;
const BRICK_COUNT: usize = BRICK_COLS * BRICK_ROWS;
const PADDLE_W: i32 = 86;
const PADDLE_SPEED: i32 = 22;
const BALL_SIZE: i32 = 12;
const GRAPHICS_FD: usize = 3;
const BREAKOUT_FRAME_MAGIC: u32 = 0x4252_4B54;
const SAVE_FILE: &str = "breakout.sav\0";

#[repr(C)]
struct BreakoutFrame {
    magic: u32,
    width: u32,
    height: u32,
    paddle_x: i32,
    ball_x: i32,
    ball_y: i32,
    bricks: [u8; BRICK_COUNT],
    score: u32,
    lives: u32,
    level: u32,
    saved: u32,
    game_over: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SaveData {
    magic: u32,
    paddle_x: i32,
    ball_x: i32,
    ball_y: i32,
    vx: i32,
    vy: i32,
    bricks: [u8; BRICK_COUNT],
    score: u32,
    lives: u32,
    level: u32,
}

struct Game {
    paddle_x: i32,
    ball_x: i32,
    ball_y: i32,
    vx: i32,
    vy: i32,
    bricks: [u8; BRICK_COUNT],
    score: u32,
    lives: u32,
    level: u32,
    saved_ticks: u32,
    game_over: bool,
}

impl Game {
    fn new() -> Self {
        Self {
            paddle_x: WIDTH / 2 - PADDLE_W / 2,
            ball_x: WIDTH / 2,
            ball_y: HEIGHT - 64,
            vx: 5,
            vy: -5,
            bricks: [1; BRICK_COUNT],
            score: 0,
            lives: 3,
            level: 1,
            saved_ticks: 0,
            game_over: false,
        }
    }

    fn reset_ball(&mut self) {
        self.paddle_x = WIDTH / 2 - PADDLE_W / 2;
        self.ball_x = WIDTH / 2;
        self.ball_y = HEIGHT - 64;
        self.vx = 4 + self.level as i32;
        self.vy = -5 - (self.level as i32 / 2);
    }

    fn reset_bricks(&mut self) {
        self.bricks = [1; BRICK_COUNT];
        self.level += 1;
        self.reset_ball();
    }

    fn submit(&self) {
        let frame = BreakoutFrame {
            magic: BREAKOUT_FRAME_MAGIC,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            paddle_x: self.paddle_x,
            ball_x: self.ball_x,
            ball_y: self.ball_y,
            bricks: self.bricks,
            score: self.score,
            lives: self.lives,
            level: self.level,
            saved: (self.saved_ticks > 0) as u32,
            game_over: self.game_over as u32,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &frame as *const BreakoutFrame as *const u8,
                core::mem::size_of::<BreakoutFrame>(),
            )
        };
        write(GRAPHICS_FD, bytes);
    }

    fn save(&mut self) {
        let fd = open(SAVE_FILE, OpenFlags::CREATE | OpenFlags::WRONLY);
        if fd < 0 {
            return;
        }
        let data = SaveData {
            magic: BREAKOUT_FRAME_MAGIC,
            paddle_x: self.paddle_x,
            ball_x: self.ball_x,
            ball_y: self.ball_y,
            vx: self.vx,
            vy: self.vy,
            bricks: self.bricks,
            score: self.score,
            lives: self.lives,
            level: self.level,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &data as *const SaveData as *const u8,
                core::mem::size_of::<SaveData>(),
            )
        };
        write(fd as usize, bytes);
        close(fd as usize);
        self.saved_ticks = 40;
    }

    fn load(&mut self) {
        let fd = open(SAVE_FILE, OpenFlags::RDONLY);
        if fd < 0 {
            return;
        }
        let mut data = SaveData {
            magic: 0,
            paddle_x: 0,
            ball_x: 0,
            ball_y: 0,
            vx: 0,
            vy: 0,
            bricks: [0; BRICK_COUNT],
            score: 0,
            lives: 0,
            level: 0,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                &mut data as *mut SaveData as *mut u8,
                core::mem::size_of::<SaveData>(),
            )
        };
        let got = read(fd as usize, bytes);
        close(fd as usize);
        if got as usize == core::mem::size_of::<SaveData>() && data.magic == BREAKOUT_FRAME_MAGIC {
            self.paddle_x = data.paddle_x;
            self.ball_x = data.ball_x;
            self.ball_y = data.ball_y;
            self.vx = data.vx;
            self.vy = data.vy;
            self.bricks = data.bricks;
            self.score = data.score;
            self.lives = data.lives;
            self.level = data.level.max(1);
            self.game_over = false;
            self.saved_ticks = 40;
        }
    }

    fn handle_key(&mut self, key: u8) -> bool {
        match key {
            b'a' | b'A' => self.paddle_x -= PADDLE_SPEED,
            b'd' | b'D' => self.paddle_x += PADDLE_SPEED,
            b's' | b'S' => self.save(),
            b'r' | b'R' => self.load(),
            b' ' if self.game_over => *self = Self::new(),
            b'q' | b'Q' => return true,
            _ => {}
        }
        self.paddle_x = self.paddle_x.max(0).min(WIDTH - PADDLE_W);
        false
    }

    fn tick(&mut self) {
        if self.saved_ticks > 0 {
            self.saved_ticks -= 1;
        }
        if self.game_over {
            return;
        }

        self.ball_x += self.vx;
        self.ball_y += self.vy;

        if self.ball_x <= 0 || self.ball_x + BALL_SIZE >= WIDTH {
            self.vx = -self.vx;
            self.ball_x = self.ball_x.max(0).min(WIDTH - BALL_SIZE);
        }
        if self.ball_y <= 0 {
            self.vy = self.vy.abs();
            self.ball_y = 0;
        }

        let paddle_y = HEIGHT - 38;
        let paddle_hit = self.ball_y + BALL_SIZE >= paddle_y
            && self.ball_y <= paddle_y + 14
            && self.ball_x + BALL_SIZE >= self.paddle_x
            && self.ball_x <= self.paddle_x + PADDLE_W;
        if paddle_hit && self.vy > 0 {
            self.vy = -self.vy.abs();
            let center = self.paddle_x + PADDLE_W / 2;
            self.vx += (self.ball_x + BALL_SIZE / 2 - center) / 18;
            self.vx = self.vx.clamp(-10, 10);
            if self.vx == 0 {
                self.vx = 3;
            }
        }

        let brick_w = (WIDTH - 40 - 9 * 5) / BRICK_COLS as i32;
        let brick_h = 18;
        let brick_x0 = 20;
        let brick_y0 = 24;
        let mut row = 0usize;
        while row < BRICK_ROWS {
            let mut col = 0usize;
            while col < BRICK_COLS {
                let index = row * BRICK_COLS + col;
                if self.bricks[index] != 0 {
                    let x = brick_x0 + col as i32 * (brick_w + 5);
                    let y = brick_y0 + row as i32 * (brick_h + 5);
                    let hit = self.ball_x + BALL_SIZE >= x
                        && self.ball_x <= x + brick_w
                        && self.ball_y + BALL_SIZE >= y
                        && self.ball_y <= y + brick_h;
                    if hit {
                        self.bricks[index] = 0;
                        self.score += 10;
                        self.vy = -self.vy;
                        if self.bricks.iter().all(|b| *b == 0) {
                            self.reset_bricks();
                        }
                        return;
                    }
                }
                col += 1;
            }
            row += 1;
        }

        if self.ball_y > HEIGHT {
            if self.lives > 0 {
                self.lives -= 1;
            }
            if self.lives == 0 {
                self.game_over = true;
            } else {
                self.reset_ball();
            }
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    let mut game = Game::new();
    let mut last_tick = get_time();
    println!("ch6 breakout: A/D move, S save, R restore, Space restart, Q quit");
    game.submit();
    loop {
        while let Some(key) = try_getchar() {
            if game.handle_key(key) {
                game.submit();
                println!("Test ch6 breakout OK!");
                return 0;
            }
        }
        let now = get_time();
        if now - last_tick >= 24 {
            last_tick = now;
            game.tick();
            game.submit();
        }
        sleep(4);
        sched_yield();
    }
}
