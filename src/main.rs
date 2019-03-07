#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

//use cortex_m_semihosting::hprintln;
use hal::gpio::{Edge, ExtiPin, Floating, Input, Output, PushPull};
use hal::prelude::*;
use hal::stm32::EXTI;
use rtfm::app;

type Led = hal::gpio::gpiod::PD<Output<PushPull>>;
type UserButton = hal::gpio::gpioa::PA0<Input<Floating>>;

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
    pub leds: [Led; 4]
}

impl LedCycle {
    const PERIOD: u32 = 8_000_000;

    fn from(leds: [Led; 4]) -> LedCycle {
        LedCycle {
            enabled: true,
            direction: LedDirection::Clockwise,
            index: 0,
            leds
        }
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn reverse(&mut self) {
        self.direction = self.direction.flip();
    }

    fn advance(&mut self) {
        let num_leds = self.leds.len();

        self.leds[self.index].set_high();
        self.leds[(self.index + 2) % num_leds].set_low();

        self.index = match self.direction {
            LedDirection::Clockwise => (self.index + 1) % num_leds,
            LedDirection::CounterClockwise => (self.index + 3) % num_leds,
        };
    }

    fn all_on(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_high();
        }
    }

    fn all_off(&mut self) {
        for led in self.leds.iter_mut() {
            led.set_low();
        }
    }
}

#[app(device = hal::stm32)]
const APP: () = {
    static mut button: UserButton = ();
    static mut led_cycle: LedCycle = ();
    static mut exti: EXTI = ();

    #[init(spawn = [switch_leds])]
    fn init() -> init::LateResources {
        // Set up the LEDs and spawn the LEDs switch task.
        let gpiod = device.GPIOD.split();
        let leds = [
            gpiod.pd12.into_push_pull_output().downgrade(),
            gpiod.pd13.into_push_pull_output().downgrade(),
            gpiod.pd14.into_push_pull_output().downgrade(),
            gpiod.pd15.into_push_pull_output().downgrade(),
        ];
        let led_cycle = LedCycle::from(leds);
        spawn.switch_leds().unwrap();

        // Set up the EXTI0 interrup for the user button.
        let mut exti = device.EXTI;
        let gpioa = device.GPIOA.split();
        let mut button = gpioa.pa0.into_floating_input();
        button.enable_interrupt(&mut exti);
        button.trigger_on_edge(&mut exti, Edge::RISING);

        init::LateResources { button, exti, led_cycle }
    }

    #[task(schedule = [switch_leds], resources = [led_cycle])]
    fn switch_leds() {
        resources.led_cycle.lock(|led_cycle| {
            if led_cycle.enabled {
                led_cycle.advance();
                schedule.switch_leds(scheduled + LedCycle::PERIOD.cycles()).unwrap();
            }
        });
    }

    #[interrupt(binds = EXTI0, resources = [button, exti, led_cycle])]
    fn button_pressed() {
        resources.led_cycle.lock(|led_cycle| led_cycle.reverse());

        resources.button.clear_interrupt_pending_bit(resources.exti);
    }

    extern "C" {
        fn TIM2();
    }
};
