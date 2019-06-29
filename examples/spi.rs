//! Accelerometer via SPI example
//!
//! This example reads all axes from the accelerometer and outputs it via semihosting
//! debug output.

#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
//use hal::block;
use hal::prelude::*;
use hal::spi::{Mode, Phase, Polarity, Spi};
#[cfg(not(test))]
use panic_semihosting as _;

#[entry]
fn main() -> ! {
    let device = hal::stm32::Peripherals::take().unwrap();

    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.freeze();

    let gpioa = device.GPIOA.split();
    let sck = gpioa.pa5.into_alternate_af5();
    let miso = gpioa.pa6.into_alternate_af5();
    let mosi = gpioa.pa7.into_alternate_af5();
    let mode = Mode {
        polarity: Polarity::IdleHigh,
        phase: Phase::CaptureOnSecondTransition,
    };
    let mut spi = Spi::spi1(device.SPI1, (sck, miso, mosi), mode, 100.hz(), clocks);

    let gpioe = device.GPIOE.split();
    let mut cs = gpioe.pe3.into_push_pull_output();

    // Init
    cs.set_low();
    let mut commands = [0x20, 0b01000111];
    let _ = spi.transfer(&mut commands[..]).unwrap();
    cs.set_high();

    loop {
        // Read
        cs.set_low();
        let mut commands = [(1 << 7) | (1 << 6) | 0x29, 0x0, 0x0, 0x0, 0x0, 0x0];
        let result = spi.transfer(&mut commands[..]).unwrap();
        let acc_x = result[1] as i8;
        let acc_y = result[3] as i8;
        let acc_z = result[5] as i8;
        cs.set_high();
        hprintln!("{}, {}, {}", acc_x, acc_y, acc_z).unwrap();
    }
}
