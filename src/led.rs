use hal::gpio::{Output, PushPull};
use hal::prelude::*;

pub type Led = hal::gpio::gpiod::PD<Output<PushPull>>;

pub enum LedDirection {
    Clockwise,
    CounterClockwise,
}

impl LedDirection {
    fn flip(&self) -> LedDirection {
        match self {
            LedDirection::Clockwise => LedDirection::CounterClockwise,
            LedDirection::CounterClockwise => LedDirection::Clockwise,
        }
    }
}

pub struct LedCycle {
    pub enabled: bool,
    pub direction: LedDirection,
    pub index: usize,
    pub leds: [crate::Led; 4],
}

impl LedCycle {
    pub const PERIOD: u32 = 8_000_000;

    pub fn from(leds: [crate::Led; 4]) -> LedCycle {
        LedCycle {
            enabled: true,
            direction: LedDirection::Clockwise,
            index: 0,
            leds,
        }
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn reverse(&mut self) {
        self.direction = self.direction.flip();
    }

    pub fn advance(&mut self) {
        let num_leds = self.leds.len();

        self.leds[self.index].set_high();
        self.leds[(self.index + 2) % num_leds].set_low();

        self.index = match self.direction {
            LedDirection::Clockwise => (self.index + 1) % num_leds,
            LedDirection::CounterClockwise => (self.index + 3) % num_leds,
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
}
