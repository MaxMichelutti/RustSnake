// snakes moves from tail to head, eventually addding a piece on the tail if food is eated
// snake moves at 4 tiles per second
// and can only change direction towards its left or its right, other inputs are ignored
use std::time::{Duration, Instant};

const INIT_SNAKE_SIZE: i32 = 4;

use std::io;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::TryRecvError;
use std::thread;
extern crate rand;
extern crate num;

extern crate termios;
use std::io::Read;
use termios::{Termios, TCSANOW, ECHO, ICANON, tcsetattr};

fn clear_screen() {
    print!("{}[2J", 27 as char);
    print!("{}[1;1H", 27 as char);
}

#[derive(Debug,Clone,PartialEq,Eq)]
enum SnakeDirection{
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Coordinates{
    x: i32,
    y: i32,
}

impl Coordinates{
    pub fn new(x: i32, y: i32) -> Coordinates{
        Coordinates { x, y }
    }

    pub fn move_left(&mut self){
        self.x -= 1;
    }
    pub fn move_right(&mut self){
        self.x += 1;
    }
    pub fn move_up(&mut self){
        self.y -= 1;
    }
    pub fn move_down(&mut self){
        self.y += 1;
    }
    pub fn get_left(&self) -> Coordinates{
        let mut res = Coordinates::new(self.x, self.y);
        res.move_left();
        res
    }
    pub fn get_right(&self) -> Coordinates{
        let mut res = Coordinates::new(self.x, self.y);
        res.move_right();
        res
    }
    pub fn get_up(&self) -> Coordinates{
        let mut res = Coordinates::new(self.x, self.y);
        res.move_up();
        res
    }
    pub fn get_down(&self) -> Coordinates{
        let mut res = Coordinates::new(self.x, self.y);
        res.move_down();
        res
    }
}

#[derive(Debug,Clone)]
pub struct SnakeGame{
    board_size: Coordinates,
    board: Vec<Vec<i32>>,
    snake_head_position: Coordinates,
    snake_body: Vec<Coordinates>, // The head is the first element
    snake_direction: SnakeDirection,
    food_position: Coordinates,
    points: i32,
    old_termios: Termios,
    new_termios: Termios,
    input_buffer: Vec<u8>,
}

impl SnakeGame{
    pub fn new(board_size: Coordinates) -> SnakeGame{
        if board_size.x < 10 || board_size.y < 10 {
            panic!("Board size must be at least 10x10");
        }
        let mut board = vec![vec![0; board_size.y as usize]; board_size.x as usize];
        let snake_head_position = Coordinates::new(6,5);
        let mut snake_body: Vec<Coordinates> = vec!();
        let mut current_position = snake_head_position.clone();
        for _i in 0..INIT_SNAKE_SIZE{
            // 1 means occupied by snake
            board[current_position.x as usize][current_position.y as usize] = 1;
            snake_body.push(current_position.clone());
            current_position.move_left();
        }
        let half_way = (board_size.x + 6)/ 2;
        board[half_way as usize][5] = 2; //initial Food position
        let termios = Termios::from_fd(0).unwrap();// 0 is file descriptor for stdin
        let new_termios = termios.clone();
        SnakeGame { 
            board_size,
            board, 
            snake_head_position, 
            snake_body, 
            snake_direction: SnakeDirection::Right, 
            food_position: Coordinates::new(half_way,5),
            points: 0,
            old_termios: termios,
            new_termios,
            input_buffer: vec!()}
    }

    pub fn play(&mut self){
        // setup stdin to be non-blocking
        self.setup_streams();
        // spawn a thread to read from stdin
        let stdin_channel = self.spawn_stdin_channel();
        // main Game Loop happens here
        let mut frame_start_time = Instant::now();
        loop{
            let duration = frame_start_time.elapsed();
            if duration.as_millis() < 250 {
                // wait for next frame
                match stdin_channel.try_recv() {
                    Ok(key) => {
                        self.add_to_input_buffer(key);
                    },
                    Err(TryRecvError::Empty) => {},
                    Err(TryRecvError::Disconnected) => panic!("Channel disconnected"),
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            // reset start
            frame_start_time = Instant::now();
            // get direction input
            let direction_to_move = self.get_direction_input();
            // apply action
            let old_tail_position = self.move_snake(direction_to_move);
            // try to eat food
            self.try_eating(old_tail_position);
            // check if the game is over due to the result of the action
            if self.is_over(){
                // println!("Game Over!");
                break;
            }
            // display the board
            clear_screen();
            self.display_board();
        }
        // reset the streams
        self.reset_streams();
        // When its over display final screen
        self.display_final_screen();
    }

    fn display_final_screen(&self){
        // display the final screen
        println!("Game Over!");
        println!("Final Score: {}", self.points);
        // debug data (remove in the future)
        // println!("Snake head position: {:?}", self.snake_head_position);
        // println!("Snake body: {:?}", self.snake_body);
        // println!("Food position: {:?}", self.food_position);
    }

    fn is_over(&self) -> bool{
        // check if the snake head is out of bounds
        if self.snake_head_position.x < 0 || self.snake_head_position.x >= self.board_size.x{
            return true;
        }
        if self.snake_head_position.y < 0 || self.snake_head_position.y >= self.board_size.y{
            return true;
        }
        // check if the snake head coords also appear in the body
        for i in 1..self.snake_body.len(){
            if self.snake_head_position.x == self.snake_body[i].x && self.snake_head_position.y == self.snake_body[i].y{
                return true;
            }
        }
        // check if the whole board is snake
        if self.points == (self.board_size.x * self.board_size.y) - INIT_SNAKE_SIZE{
            return true;
        }
        false
    }

    fn display_board(&self){
        //border up
        for _i in 0..self.board_size.x + 2{
            print!("* ");
        }
        println!();
        for i in 0..self.board_size.y{
            //border left
            print!("* ");
            for j in 0..self.board_size.x{
                let item = self.board[j as usize][i as usize];
                if item == 0{
                    print!("  ");
                }else if item == 1{
                    print!("O ");
                }else if item == 2{
                    print!("F ");
                }else{
                    panic!("Invalid item on the board!");
                }
            }
            //border right
            print!("* ");
            //newline
            println!();
        }
        //border down
        for _i in 0..self.board_size.x + 2{
            print!("* ");
        }
        println!();
        // score
        println!("Points: {}", self.points);
        // debug info (disable on release)
        // println!("Snake head position: {:?}", self.snake_head_position);
        // println!("Snake body: {:?}", self.snake_body);
        // println!("Food position: {:?}", self.food_position);
        // println!("Input buffer: {:?}", self.input_buffer);
    }

    fn get_direction_input(&mut self) -> SnakeDirection{
        let default_direction = self.snake_direction.clone();
        let mut result = default_direction.clone();
        // arrows keys are long 3 bytes, first 2 need to be 27 and 91
        while self.input_buffer.len() >= 3 {// find the first arrow key in the buffer
            let mut found = false;
            if self.input_buffer[0] == 27 && self.input_buffer[1] == 91{
                match self.input_buffer[2]{
                    65 => { // up arrow
                        if result != SnakeDirection::Down{
                            result = SnakeDirection::Up;
                        }
                        found = true;
                    },
                    66 => { // down arrow
                        if result != SnakeDirection::Up{
                            result = SnakeDirection::Down;
                        }
                        found = true;
                    },
                    67 => { // right arrow
                        if result != SnakeDirection::Left{
                            result = SnakeDirection::Right;
                        }
                        found = true;
                    },
                    68 => { // left arrow
                        if result != SnakeDirection::Right{
                            result = SnakeDirection::Left;
                        }
                        found = true;
                    },
                    _ => {}// not an arrow
                }
            }
            // remove head of the buffer
            self.input_buffer.remove(0);
            // check if input was found
            if found {
                break;
            }
        }
        // clear the input buffer
        self.input_buffer.clear();
        result
    }

    fn spawn_stdin_channel(&self) -> Receiver<u8> {
        let (tx, rx) = mpsc::channel::<u8>();
        thread::spawn(move || loop {
            let mut reader = io::stdin();
            let mut buffer: [u8; 1] = [1;1];
            reader.read_exact(&mut buffer).unwrap();
            tx.send(buffer[0]).unwrap();
        });
        rx
    }

    fn setup_streams(&mut self) {
        self.new_termios.c_lflag &= !(ICANON | ECHO); // no echo and canonical mode for stdin
        tcsetattr(0, TCSANOW, &mut self.new_termios).unwrap();
    }
    
    fn reset_streams(&mut self) {
        tcsetattr(0, TCSANOW, & self.old_termios).unwrap();
    }

    fn add_to_input_buffer(&mut self, key: u8){
        if key == 1{//1 is the default value that should be ignored
            return;
        }
        // I should also check to ignore all keys that are not inputs for the game
        self.input_buffer.push(key);
    }

    fn move_snake(&mut self, direction: SnakeDirection) -> Coordinates{
        self.snake_direction = direction.clone();
        let new_head = match direction{
            SnakeDirection::Up => self.snake_head_position.get_up(),
            SnakeDirection::Down => self.snake_head_position.get_down(),
            SnakeDirection::Left => self.snake_head_position.get_left(),
            SnakeDirection::Right => self.snake_head_position.get_right(),
        };
        self.snake_head_position = new_head.clone();
        self.snake_body.insert(0, new_head.clone());
        // update the board
        if new_head.x >= 0 && new_head.x < self.board_size.x && 
            new_head.y >= 0 && new_head.y < self.board_size.y {
            // if the new head position is valid, update the board for new head, 
            // otherwise the game will eventually exit anyway
            self.board[new_head.x as usize][new_head.y as usize] = 1;
        }
        let old_tail = self.snake_body.pop().unwrap();
        self.board[old_tail.x as usize][old_tail.y as usize] = 0;
        old_tail
    }

    fn try_eating(&mut self, old_tail: Coordinates){
        if self.snake_head_position == self.food_position{
            //the food is eaten
            // increment score
            self.points += 1;
            // add the tail back to the snake
            self.snake_body.push(old_tail.clone());
            self.board[old_tail.x as usize][old_tail.y as usize] = 1;
            // exit the game if board is full
            if self.is_over(){
                return;
            }
            // try to gen new food
            self.generate_food();
        }
    }

    fn generate_food(&mut self){
        let board_size = self.board_size.x * self.board_size.y;
        let snake_length = self.snake_body.len() as i32;
        if self.is_over(){//make sure that I am able to gen food
            return;
        }
        // if snake occupies more than 80% of the board, use vec, 
        // else use random choice of empty space
        if snake_length > (board_size * 4) / 5{
            // use vec
            let mut empty_positions: Vec<Coordinates> = vec!();
            for i in 0..self.board_size.x{
                for j in 0..self.board_size.y{
                    if self.board[i as usize][j as usize] == 0{
                        empty_positions.push(Coordinates::new(i,j));
                    }
                }
            }
            // choose a random position from the empty positions
            let random_index = num::abs(rand::random::<i32>()) % (empty_positions.len() as i32);
            println!("Random index: {}", random_index);
            self.food_position = empty_positions[random_index as usize].clone();
            self.board[self.food_position.x as usize][self.food_position.y as usize] = 2;
        }else{
            // use random choice
            loop{
                let random_x = num::abs(rand::random::<i32>()) % self.board_size.x;
                let random_y = num::abs(rand::random::<i32>()) % self.board_size.y;
                println!("Random x: {}, y: {}", random_x, random_y);
                if self.board[random_x as usize][random_y as usize] == 0{
                    self.food_position = Coordinates::new(random_x, random_y);
                    self.board[random_x as usize][random_y as usize] = 2;
                    break;
                }
            }
        }
    }
}