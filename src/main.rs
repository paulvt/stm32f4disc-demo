#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

mod led;

use crate::led::{Led, LedCycle};
use core::fmt::Write;
use cortex_m_semihosting::hprintln;
use hal::gpio::{Edge, ExtiPin, Floating, Input};
use hal::prelude::*;
use hal::serial::{self, config::Config as SerialConfig, Serial};
use hal::stm32::{EXTI, USART1};
use heapless::consts::U8;
use heapless::Vec;
use rtfm::app;

type SerialTx = hal::serial::Tx<USART1>;
type SerialRx = hal::serial::Rx<USART1>;
type UserButton = hal::gpio::gpioa::PA0<Input<Floating>>;

#[app(device = hal::stm32)]
const APP: () = {
    static mut button: UserButton = ();
    static mut buffer: Vec<u8, U8> = ();
    static mut led_cycle: LedCycle = ();
    static mut exti: EXTI = ();
    static mut serial_rx: SerialRx = ();
    static mut serial_tx: SerialTx = ();

    #[init(spawn = [switch_leds])]
    fn init() -> init::LateResources {
        // Set up the LED cycle and spawn the LEDs switch task.
        let gpiod = device.GPIOD.split();
        let leds = [
            gpiod.pd12.into_push_pull_output().downgrade(),
            gpiod.pd13.into_push_pull_output().downgrade(),
            gpiod.pd14.into_push_pull_output().downgrade(),
            gpiod.pd15.into_push_pull_output().downgrade(),
        ];
        let led_cycle = LedCycle::from(leds);
        spawn.switch_leds().unwrap();

        // Set up the EXTI0 interrupt for the user button.
        let mut exti = device.EXTI;
        let gpioa = device.GPIOA.split();
        let mut button = gpioa.pa0.into_floating_input();
        button.enable_interrupt(&mut exti);
        button.trigger_on_edge(&mut exti, Edge::RISING);

        // Set up the serial interface.
        let tx = gpioa.pa9.into_alternate_af7();
        let rx = gpioa.pa10.into_alternate_af7();
        let config = SerialConfig::default();
        let rcc = device.RCC.constrain();
        let clocks = rcc.cfgr.freeze();
        let mut serial = Serial::usart1(device.USART1, (tx, rx), config, clocks).unwrap();
        serial.listen(serial::Event::Rxne);
        let (serial_tx, serial_rx) = serial.split();

        // Set up the serial interface command buffer.
        let buffer = Vec::new();

        init::LateResources {
            button,
            buffer,
            exti,
            led_cycle,
            serial_tx,
            serial_rx,
        }
    }

    #[task(schedule = [switch_leds], resources = [led_cycle])]
    fn switch_leds() {
        resources.led_cycle.lock(|led_cycle| {
            if led_cycle.enabled {
                led_cycle.advance();
                schedule
                    .switch_leds(scheduled + LedCycle::PERIOD.cycles())
                    .unwrap();
            }
        });
    }

    #[interrupt(binds = EXTI0, resources = [button, exti, led_cycle])]
    fn button_pressed() {
        resources.led_cycle.lock(|led_cycle| led_cycle.reverse());

        resources.button.clear_interrupt_pending_bit(resources.exti);
    }

    #[interrupt(
        binds = USART1,
        priority = 2,
        resources = [buffer, led_cycle, serial_rx, serial_tx],
        spawn = [switch_leds]
    )]
    fn handle_serial() {
        let buffer = resources.buffer;

        // Read a byte from the serial port and write it back.
        let byte = resources.serial_rx.read().unwrap();
        resources.serial_tx.write(byte).unwrap();
        //hprintln!("serial: {}", byte).unwrap();

        // Handle the command in the buffer for newline, otherwise append to the buffer.
        if byte == b'\r' {
            match &buffer[..] {
                b"flip" => {
                    resources.led_cycle.reverse();
                }
                b"stop" => {
                    resources.led_cycle.disable();
                }
                b"start" => {
                    resources.led_cycle.enable();
                    spawn.switch_leds().unwrap();
                }
                b"off" => {
                    resources.led_cycle.disable();
                    resources.led_cycle.all_off();
                }
                b"on" => {
                    resources.led_cycle.disable();
                    resources.led_cycle.all_on();
                }
                _ => {}
            }

            buffer.clear();
        } else {
            if buffer.push(byte).is_err() {
                hprintln!("Serial read buffer full!").unwrap();
            }
            //hprintln!("buffer: {:?}", buffer).unwrap();
        }
    }

    extern "C" {
        fn TIM2();
    }
};
