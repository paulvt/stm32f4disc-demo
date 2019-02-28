#![deny(unsafe_code)]
#![deny(warnings)]
#![no_std]
#![no_main]

#[cfg(debug_assertions)]
extern crate panic_semihosting;

#[cfg(not(debug_assertions))]
extern crate panic_abort;

use cortex_m_rt::entry;
//use cortex_m_semihosting::hprintln;
use hal::delay::Delay;
use hal::prelude::*;

#[entry]
fn main() -> ! {
    // Get handles to the hardware.
    let core = cortex_m::Peripherals::take().unwrap();
    let device = hal::stm32::Peripherals::take().unwrap();

    // Get a clock for the delay.
    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.freeze();
    let mut delay = Delay::new(core.SYST, clocks);

    // Set up the LEDs.
    let gpiod = device.GPIOD.split();
    let mut leds = [
        gpiod.pd12.into_push_pull_output().downgrade(),
        gpiod.pd13.into_push_pull_output().downgrade(),
        gpiod.pd14.into_push_pull_output().downgrade(),
        gpiod.pd15.into_push_pull_output().downgrade(),
    ];
    let num_leds = leds.len();
    assert_eq!(num_leds, 4);

    // Blink the LED...
    loop {
        for index in 0..num_leds {
            leds[index].set_high();
            leds[(index + 1) % num_leds].set_low();
            delay.delay_ms(500u16);
        }
    }
}
