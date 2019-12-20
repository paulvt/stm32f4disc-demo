//! Module for manipulating the LED ring.

use core::convert::Infallible;
use hal::prelude::_embedded_hal_digital_v2_OutputPin as OutputPin;

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
    LED: OutputPin<Error = Infallible>,
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

        self.leds[self.index].set_high().unwrap();
        self.leds[(self.index + 2) % num_leds].set_low().unwrap();

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
            led.set_high().unwrap();
        }
    }

    /// Turns all LEDs off.
    ///
    /// This is done immediately, regardless of the current mode.
    pub fn all_off(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_low().unwrap();
        }
    }

    /// Turns on specific LEDs based on the "direction" array.
    ///
    /// When looking with the mini-USB port of the board held down (south), the directions of
    /// the array can be interpreted as: `[east, south, west, north]`.
    pub fn specific_on(&mut self, directions: [bool; 4]) {
        for (led, on_off) in self.leds.iter_mut().zip(directions.iter()) {
            if *on_off {
                led.set_high().unwrap();
            } else {
                led.set_low().unwrap();
            }
        }
    }

    /// Provides access to the LEDs (for testing purposes only).
    #[cfg(test)]
    pub fn leds_mut(&self) -> &[LED; 4] {
        &self.leds
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, Infallible, LedRing, Mode, OutputPin};

    #[derive(Debug, Eq, PartialEq)]
    struct MockOutputPin {
        state: bool,
    }

    impl MockOutputPin {
        fn get_4() -> [Self; 4] {
            [
                Self { state: false },
                Self { state: false },
                Self { state: false },
                Self { state: false },
            ]
        }
    }

    impl OutputPin for MockOutputPin {
        type Error = Infallible;

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.state = true;
            Ok(())
        }

        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.state = false;
            Ok(())
        }
    }

    macro_rules! assert_pins {
        ($pins:expr, [$pin0:expr, $pin1:expr, $pin2:expr, $pin3:expr]) => {{
            assert_eq!($pins[0].state, $pin0, "(mock pin 0)");
            assert_eq!($pins[1].state, $pin1, "(mock pin 1)");
            assert_eq!($pins[2].state, $pin2, "(mock pin 2)");
            assert_eq!($pins[3].state, $pin3, "(mock pin 3)");
        }};
    }

    #[test]
    fn direction_flip() {
        let cw_dir = Direction::Clockwise;
        assert_eq!(cw_dir.flip(), Direction::CounterClockwise);
        let ccw_dir = Direction::CounterClockwise;
        assert_eq!(ccw_dir.flip(), Direction::Clockwise);
    }

    #[test]
    fn led_ring_init() {
        let mock_leds = MockOutputPin::get_4();
        let led_ring = LedRing::<MockOutputPin>::from(mock_leds);

        assert_eq!(led_ring.direction(), Direction::Clockwise);
        assert_eq!(led_ring.mode(), Mode::Cycle);
    }

    #[test]
    fn led_ring_mode() {
        let mock_leds = MockOutputPin::get_4();
        let mut led_ring = LedRing::<MockOutputPin>::from(mock_leds);

        led_ring.enable_accel();
        assert_eq!(led_ring.mode(), Mode::Accelerometer);
        assert!(led_ring.is_mode_accel());
        assert!(!led_ring.is_mode_cycle());

        led_ring.disable();
        assert_eq!(led_ring.mode(), Mode::Off);
        assert!(!led_ring.is_mode_accel());
        assert!(!led_ring.is_mode_cycle());

        led_ring.enable_cycle();
        assert_eq!(led_ring.mode(), Mode::Cycle);
        assert!(!led_ring.is_mode_accel());
        assert!(led_ring.is_mode_cycle());
    }

    #[test]
    fn led_ring_direction() {
        let mock_leds = MockOutputPin::get_4();
        let mut led_ring = LedRing::<MockOutputPin>::from(mock_leds);

        led_ring.reverse();
        assert_eq!(led_ring.direction(), Direction::CounterClockwise);

        led_ring.reverse();
        assert_eq!(led_ring.direction(), Direction::Clockwise);
    }

    #[test]
    fn led_ring_advance() {
        let mock_leds = MockOutputPin::get_4();
        let mut led_ring = LedRing::<MockOutputPin>::from(mock_leds);

        assert_pins!(led_ring.leds_mut(), [false, false, false, false]);
        led_ring.advance();
        assert_pins!(led_ring.leds_mut(), [true, false, false, false]);
        led_ring.advance();
        assert_pins!(led_ring.leds_mut(), [true, true, false, false]);
        led_ring.advance();
        assert_pins!(led_ring.leds_mut(), [false, true, true, false]);
        led_ring.advance();
        assert_pins!(led_ring.leds_mut(), [false, false, true, true]);
        led_ring.advance();
        assert_pins!(led_ring.leds_mut(), [true, false, false, true]);
        led_ring.advance();
        assert_pins!(led_ring.leds_mut(), [true, true, false, false]);
        led_ring.advance();
    }

    #[test]
    fn led_ring_all_on_off() {
        let mock_leds = MockOutputPin::get_4();
        let mut led_ring = LedRing::<MockOutputPin>::from(mock_leds);

        assert_pins!(led_ring.leds_mut(), [false, false, false, false]);
        led_ring.all_on();
        assert_pins!(led_ring.leds_mut(), [true, true, true, true]);
        led_ring.all_off();
        assert_pins!(led_ring.leds_mut(), [false, false, false, false]);
    }

    #[test]
    fn led_ring_specific_on() {
        let mock_leds = MockOutputPin::get_4();
        let mut led_ring = LedRing::<MockOutputPin>::from(mock_leds);

        assert_pins!(led_ring.leds_mut(), [false, false, false, false]);
        led_ring.specific_on([true, false, true, false]);
        assert_pins!(led_ring.leds_mut(), [true, false, true, false]);
    }
}
