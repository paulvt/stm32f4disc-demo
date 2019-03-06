#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

//use cortex_m_semihosting::hprintln;
use hal::gpio::{Edge, ExtiPin, Floating, Input, Output, PushPull};
use hal::prelude::*;
use rtfm::app;

type Led = hal::gpio::gpiod::PD<Output<PushPull>>;
type UserButton = hal::gpio::gpioa::PA0<Input<Floating>>;

const PERIOD: u32 = 8_000_000;

pub enum LedDirection {
    Clockwise,
    CounterClockwise,
}

#[app(device = hal::stm32)]
const APP: () = {
    static mut button: UserButton = ();
    static mut leds: [Led; 4] = ();
    static mut led_cycle_direction: LedDirection = LedDirection::Clockwise;
    static mut led_index: usize = 0;
    static mut exti: hal::stm32::EXTI = ();

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
        spawn.switch_leds().unwrap();

        // Set up the EXTI0 interrup for the user button.
        let mut exti = device.EXTI;
        let gpioa = device.GPIOA.split();
        let mut button = gpioa.pa0.into_floating_input();
        button.enable_interrupt(&mut exti);
        button.trigger_on_edge(&mut exti, Edge::RISING);

        init::LateResources { button, exti, leds }
    }

    #[task(schedule = [switch_leds],
        resources = [led_index, led_cycle_direction, leds])]
    fn switch_leds() {
        let led_index = *resources.led_index;
        let num_leds = resources.leds.len();

        resources.leds[led_index].set_high();
        resources.leds[(led_index + 2) % num_leds].set_low();
        *resources.led_index = match *resources.led_cycle_direction {
            LedDirection::Clockwise => (led_index + 1) % num_leds,
            LedDirection::CounterClockwise => (led_index + 3) % num_leds,
        };

        schedule.switch_leds(scheduled + PERIOD.cycles()).unwrap();
    }

    #[interrupt(binds = EXTI0, resources = [button, exti, led_cycle_direction])]
    fn button_pressed() {
        *resources.led_cycle_direction = match *resources.led_cycle_direction {
            LedDirection::Clockwise => LedDirection::CounterClockwise,
            LedDirection::CounterClockwise => LedDirection::Clockwise,
        };
        resources.button.clear_interrupt_pending_bit(resources.exti);
    }

    extern "C" {
        fn UART4();
    }
};
