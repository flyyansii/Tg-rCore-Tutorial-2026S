#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{sched_yield, sleep, try_getchar, write};

const MAP_W: usize = 19;
const MAP_H: usize = 15;
const MAP_SIZE: usize = MAP_W * MAP_H;
const GRAPHICS_FD: usize = 3;
const PACMAN_FRAME_MAGIC: u32 = 0x5041_434D;
const SCRIPTED_DEMO: bool = true;

const RAW_MAP: [&[u8; MAP_W]; MAP_H] = [
    b"###################",
    b"#........#........#",
    b"#.###.##.#.##.###.#",
    b"#.................#",
    b"#.##.#.#####.#.##.#",
    b"#....#...#...#....#",
    b"####.### # ###.####",
    b"   #.#       #.#   ",
    b"####.# ## ## #.####",
    b"#........#........#",
    b"#.##.###.#.###.##.#",
    b"#..#.....P.....#..#",
    b"##.#.#.#####.#.#.##",
    b"#....#.......#....#",
    b"###################",
];

#[repr(C)]
struct PacmanFrame {
    magic: u32,
    tick: u32,
    pac_x: u32,
    pac_y: u32,
    ghost_x: u32,
    ghost_y: u32,
    score: u32,
    lives: u32,
    dots_left: u32,
    game_over: u32,
    win: u32,
    map: [u8; MAP_SIZE],
}

#[derive(Clone, Copy)]
struct Pos {
    x: usize,
    y: usize,
}

struct Game {
    map: [u8; MAP_SIZE],
    pac: Pos,
    ghost: Pos,
    dir: (isize, isize),
    score: u32,
    lives: u32,
    dots_left: u32,
    tick: u32,
    game_over: bool,
    win: bool,
}

impl Game {
    fn new() -> Self {
        let mut map = [0u8; MAP_SIZE];
        let mut pac = Pos { x: 9, y: 11 };
        let mut dots_left = 0;
        let mut y = 0;
        while y < MAP_H {
            let mut x = 0;
            while x < MAP_W {
                let cell = RAW_MAP[y][x];
                map[y * MAP_W + x] = match cell {
                    b'#' => 1,
                    b'.' => {
                        dots_left += 1;
                        2
                    }
                    b'P' => {
                        pac = Pos { x, y };
                        0
                    }
                    _ => 0,
                };
                x += 1;
            }
            y += 1;
        }
        Self {
            map,
            pac,
            ghost: Pos { x: 9, y: 7 },
            dir: (1, 0),
            score: 0,
            lives: 3,
            dots_left,
            tick: 0,
            game_over: false,
            win: false,
        }
    }

    fn index(x: usize, y: usize) -> usize {
        y * MAP_W + x
    }

    fn passable(&self, x: isize, y: isize) -> bool {
        if x < 0 || y < 0 || x >= MAP_W as isize || y >= MAP_H as isize {
            return false;
        }
        self.map[Self::index(x as usize, y as usize)] != 1
    }

    fn apply_key(&mut self, key: u8) -> bool {
        match key {
            b'w' | b'W' => self.dir = (0, -1),
            b'a' | b'A' => self.dir = (-1, 0),
            b's' | b'S' => self.dir = (0, 1),
            b'd' | b'D' => self.dir = (1, 0),
            b'q' | b'Q' => return true,
            _ => {}
        }
        false
    }

    fn scripted_key(step: u32) -> u8 {
        const SCRIPT: &[u8] = b"ddddddddwwwwaaaassssddddwwwwaaaassssddddwwwwaaaassss";
        SCRIPT[(step as usize) % SCRIPT.len()]
    }

    fn move_pacman(&mut self) {
        let nx = self.pac.x as isize + self.dir.0;
        let ny = self.pac.y as isize + self.dir.1;
        if self.passable(nx, ny) {
            self.pac = Pos {
                x: nx as usize,
                y: ny as usize,
            };
        }
        let idx = Self::index(self.pac.x, self.pac.y);
        if self.map[idx] == 2 {
            self.map[idx] = 0;
            self.score += 10;
            self.dots_left -= 1;
            self.win = self.dots_left == 0;
        }
    }

    fn move_ghost(&mut self) {
        if self.tick % 2 != 0 {
            return;
        }
        let choices = [
            (self.pac.x as isize - self.ghost.x as isize).signum(),
            (self.pac.y as isize - self.ghost.y as isize).signum(),
        ];
        let candidates = [
            (choices[0], 0),
            (0, choices[1]),
            (-choices[0], 0),
            (0, -choices[1]),
        ];
        let mut i = 0;
        while i < candidates.len() {
            let (dx, dy) = candidates[i];
            let nx = self.ghost.x as isize + dx;
            let ny = self.ghost.y as isize + dy;
            if (dx != 0 || dy != 0) && self.passable(nx, ny) {
                self.ghost = Pos {
                    x: nx as usize,
                    y: ny as usize,
                };
                return;
            }
            i += 1;
        }
    }

    fn collide(&mut self) {
        if self.pac.x == self.ghost.x && self.pac.y == self.ghost.y {
            if self.lives > 1 {
                self.lives -= 1;
                self.pac = Pos { x: 9, y: 11 };
                self.ghost = Pos { x: 9, y: 7 };
                self.dir = (1, 0);
            } else {
                self.lives = 0;
                self.game_over = true;
            }
        }
    }

    fn tick(&mut self) {
        if self.game_over || self.win {
            return;
        }
        self.tick += 1;
        self.move_pacman();
        self.move_ghost();
        self.collide();
    }

    fn submit(&self) {
        let frame = PacmanFrame {
            magic: PACMAN_FRAME_MAGIC,
            tick: self.tick,
            pac_x: self.pac.x as u32,
            pac_y: self.pac.y as u32,
            ghost_x: self.ghost.x as u32,
            ghost_y: self.ghost.y as u32,
            score: self.score,
            lives: self.lives,
            dots_left: self.dots_left,
            game_over: self.game_over as u32,
            win: self.win as u32,
            map: self.map,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &frame as *const PacmanFrame as *const u8,
                core::mem::size_of::<PacmanFrame>(),
            )
        };
        write(GRAPHICS_FD, bytes);
    }
}

#[unsafe(no_mangle)]
extern "C" fn main() -> i32 {
    println!("ch7 pacman demo: scripted route, auto exit");
    let mut game = Game::new();
    let mut steps = 0u32;
    loop {
        // Draw before polling input so the first frame appears even when no key has arrived yet.
        game.submit();
        if SCRIPTED_DEMO {
            game.apply_key(Game::scripted_key(steps));
        } else if let Some(key) = try_getchar() {
            if game.apply_key(key) {
                break;
            }
        }
        game.tick();
        if game.game_over || game.win {
            game.submit();
            println!("Test ch7 pacman OK!");
            sleep(800);
            break;
        }
        steps += 1;
        if steps >= 160 {
            game.submit();
            println!("Test ch7 pacman OK!");
            return 0;
        }
        sleep(80);
        sched_yield();
    }
    0
}
