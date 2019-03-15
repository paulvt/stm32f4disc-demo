use hal::gpio::{Output, PushPull};
use hal::prelude::*;

pub type Led = hal::gpio::gpiod::PD<Output<PushPull>>;

#[derive(Debug, Eq, PartialEq)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

impl Direction {
    fn flip(&self) -> Direction {
        match self {
            Direction::Clockwise => Direction::CounterClockwise,
            Direction::CounterClockwise => Direction::Clockwise,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Mode {
    Off,
    Cycle,
    Accelerometer
}

pub struct LedRing {
    pub direction: Direction,
    pub mode: Mode,
    pub index: usize,
    pub leds: [crate::Led; 4],
}

impl LedRing {
    pub const PERIOD: u32 = 8_000_000;

    pub fn from(leds: [crate::Led; 4]) -> LedRing {
        LedRing {
            direction: Direction::Clockwise,
            mode: Mode::Cycle,
            index: 0,
            leds,
        }
    }

    pub fn enable_cycle(&mut self) {
        self.mode = Mode::Cycle;
    }

    pub fn enable_accel(&mut self) {
        self.mode = Mode::Accelerometer;
    }

    pub fn disable(&mut self) {
        self.mode = Mode::Off;
    }

    pub fn is_mode_cycle(&self) -> bool {
        self.mode == Mode::Cycle
    }

    pub fn is_mode_accel(&self) -> bool {
        self.mode == Mode::Accelerometer
    }

    pub fn reverse(&mut self) {
        self.direction = self.direction.flip();
    }

    pub fn advance(&mut self) {
        let num_leds = self.leds.len();

        self.leds[self.index].set_high();
        self.leds[(self.index + 2) % num_leds].set_low();

        self.index = match self.direction {
            Direction::Clockwise => (self.index + 1) % num_leds,
            Direction::CounterClockwise => (self.index + 3) % num_leds,
        };
    }

    pub fn all_on(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_high();
        }
    }

    pub fn all_off(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_low();
        }
    }

    pub fn set_directions(&mut self, directions: [bool; 4]) {
        for setting in self.leds.iter_mut().zip(directions.iter()) {
            let (led, on_off) = setting;
            if *on_off {
                led.set_high();
            } else {
                led.set_low();
            }
        }
    }
}
