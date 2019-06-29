//! Serial interface echo server example
//!
//! In this example every received byte will be sent back to the sender. You can test this
//! example with serial terminal emulator like `minicom`.

#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
//use cortex_m_semihosting::hprintln;
use hal::block;
use hal::prelude::*;
use hal::serial::{config::Config as SerialConfig, Serial};
#[cfg(not(test))]
use panic_semihosting as _;

#[entry]
fn main() -> ! {
    let device = hal::stm32::Peripherals::take().unwrap();

    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(48.mhz()).freeze();

    let gpioa = device.GPIOA.split();
    let tx = gpioa.pa2.into_alternate_af7();
    let rx = gpioa.pa3.into_alternate_af7();
    let config = SerialConfig::default().baudrate(9_600.bps());

    let serial = Serial::usart2(device.USART2, (tx, rx), config, clocks).unwrap();
    let (mut tx, mut rx) = serial.split();

    loop {
        let byte = block!(rx.read()).unwrap();
        //hprintln!("in: {}", byte).unwrap();
        block!(tx.write(byte)).ok();
    }
}
