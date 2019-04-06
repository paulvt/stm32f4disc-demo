//! Module for manipulating the LED ring.

use hal::prelude::_embedded_hal_digital_OutputPin as OutputPin;

/// The cycle direction of the LED ring.
///
/// The direction can be interpreted as such when the mini-USB port of the board is being held
/// down.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Cycle clockwise.
    Clockwise,
    /// Cycle counter-clockwise.
    CounterClockwise,
}

impl Direction {
    /// Returns the flipped/reversed direction.
    fn flip(&self) -> Direction {
        match self {
            Direction::Clockwise => Direction::CounterClockwise,
            Direction::CounterClockwise => Direction::Clockwise,
        }
    }
}

/// The mode the LED ring is in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// All LEDs are off.
    Off,
    /// The LEDs are cycling (two on at any time) following some direction.
    Cycle,
    /// The LEDs follow the accelerometer (shows which side of the board is pointing down).
    Accelerometer,
}

/// The LED ring.
///
/// The ring on this board is comprised of four LEDs (output pins).  This struct provides methods
/// for animating them.
pub struct LedRing<LED> {
    /// The current cycle direction.
    direction: Direction,
    /// The current mode.
    mode: Mode,
    /// The index of the current LED being lit.
    index: usize,
    /// The LED outputs being used to comprise the LED ring.
    leds: [LED; 4],
}

impl<LED> LedRing<LED>
where
    LED: OutputPin,
{
    /// Sets up the LED ring using using four LED GPIO outputs.
    pub fn from(leds: [LED; 4]) -> LedRing<LED> {
        LedRing {
            direction: Direction::Clockwise,
            mode: Mode::Cycle,
            index: 0,
            leds,
        }
    }

    /// Returns the current cycle mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Enables cycle mode.
    pub fn enable_cycle(&mut self) {
        self.mode = Mode::Cycle;
    }

    /// Enables accelerometer mode.
    pub fn enable_accel(&mut self) {
        self.mode = Mode::Accelerometer;
    }

    /// Disables either cycle or accelerometer mode.
    pub fn disable(&mut self) {
        self.mode = Mode::Off;
    }

    /// Returns whether the LED ring is in cycle mode.
    pub fn is_mode_cycle(&self) -> bool {
        self.mode == Mode::Cycle
    }

    /// Returns whether the LED ring is in acceleromter mode.
    pub fn is_mode_accel(&self) -> bool {
        self.mode == Mode::Accelerometer
    }

    /// Returns the current cycle direction.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Reverses the cycle direction.
    ///
    /// This will have no immediately visible effect if the LED ring is not in cycle mode
    /// but it will be used when the cycle mode is enabled again.
    pub fn reverse(&mut self) {
        self.direction = self.direction.flip();
    }

    /// Advances the cycling one step.
    ///
    /// This will have have directly visible effect regardless of the mode the
    /// LED ring is in and override what is shown at that moment.
    pub fn advance(&mut self) {
        let num_leds = self.leds.len();

        self.leds[self.index].set_high();
        self.leds[(self.index + 2) % num_leds].set_low();

        self.index = match self.direction {
            Direction::Clockwise => (self.index + 1) % num_leds,
            Direction::CounterClockwise => (self.index + 3) % num_leds,
        };
    }

    /// Turns all LEDs on.
    ///
    /// This is done immediately, regardless of the current mode.
    pub fn all_on(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_high();
        }
    }

    /// Turns all LEDs off.
    ///
    /// This is done immediately, regardless of the current mode.
    pub fn all_off(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_low();
        }
    }

    /// Turns on specific LEDs based on the "direction" array.
    ///
    /// When looking with the mini-USB port of the board held down (south), the directions of
    /// the array can be interpreted as: `[east, south, west, north]`.
    pub fn specific_on(&mut self, directions: [bool; 4]) {
        for (led, on_off) in self.leds.iter_mut().zip(directions.iter()) {
            if *on_off {
                led.set_high();
            } else {
                led.set_low();
            }
        }
    }
}
