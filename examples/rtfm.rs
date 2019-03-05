//! examples/rtfm.rs

#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

//use cortex_m_semihosting::hprintln;
use hal::gpio::{Output, PushPull};
use hal::prelude::*;
use rtfm::{app, Instant};

type Led = hal::gpio::gpiod::PD<Output<PushPull>>;

const PERIOD: u32 = 8_000_000;

#[app(device = hal::stm32)]
const APP: () = {
    static mut index: usize = ();
    static mut leds: [Led; 4] = ();

    #[init(schedule = [switch_leds])]
    fn init() -> init::LateResources {
        // Set up the LEDs.
        let gpiod = device.GPIOD.split();
        let leds = [
            gpiod.pd12.into_push_pull_output().downgrade(),
            gpiod.pd13.into_push_pull_output().downgrade(),
            gpiod.pd14.into_push_pull_output().downgrade(),
            gpiod.pd15.into_push_pull_output().downgrade(),
        ];

        schedule.switch_leds(Instant::now() + PERIOD.cycles()).unwrap();

        init::LateResources { index: 0, leds }
    }

    #[task(schedule = [switch_leds], resources = [index, leds])]
    fn switch_leds() {
        let index = *resources.index;
        let num_leds = resources.leds.len();
        resources.leds[index].set_high();
        resources.leds[(index + 2) % num_leds].set_low();
        *resources.index = (index + 1) % 4;

        schedule.switch_leds(scheduled + PERIOD.cycles()).unwrap();
    }

    extern "C" {
        fn UART4();
    }
};
