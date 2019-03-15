#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

extern crate panic_semihosting;

mod led;

use crate::led::{Led, LedRing};
use core::fmt::Write;
use cortex_m_semihosting::hprintln;
use hal::block;
use hal::gpio::{Edge, ExtiPin, Floating, Input};
use hal::prelude::*;
use hal::serial::{self, config::Config as SerialConfig, Serial};
use hal::stm32::{EXTI, USART2};
use heapless::consts::U8;
use heapless::Vec;
use rtfm::app;

type SerialTx = hal::serial::Tx<USART2>;
type SerialRx = hal::serial::Rx<USART2>;
type UserButton = hal::gpio::gpioa::PA0<Input<Floating>>;

#[app(device = hal::stm32)]
const APP: () = {
    static mut button: UserButton = ();
    static mut buffer: Vec<u8, U8> = ();
    static mut led_ring: LedRing = ();
    static mut exti: EXTI = ();
    static mut serial_rx: SerialRx = ();
    static mut serial_tx: SerialTx = ();

    #[init(spawn = [switch_leds])]
    fn init() -> init::LateResources {
        // Set up the LED ring and spawn the LEDs switch task.
        let gpiod = device.GPIOD.split();
        let leds = [
            gpiod.pd12.into_push_pull_output().downgrade(),
            gpiod.pd13.into_push_pull_output().downgrade(),
            gpiod.pd14.into_push_pull_output().downgrade(),
            gpiod.pd15.into_push_pull_output().downgrade(),
        ];
        let led_ring = LedRing::from(leds);
        spawn.switch_leds().unwrap();

        // Set up the EXTI0 interrupt for the user button.
        let mut exti = device.EXTI;
        let gpioa = device.GPIOA.split();
        let mut button = gpioa.pa0.into_floating_input();
        button.enable_interrupt(&mut exti);
        button.trigger_on_edge(&mut exti, Edge::RISING);

        // Set up the serial interface and the USART2 interrupt.
        let tx = gpioa.pa2.into_alternate_af7();
        let rx = gpioa.pa3.into_alternate_af7();
        let config = SerialConfig::default().baudrate(115_200.bps());
        let rcc = device.RCC.constrain();
        let clocks = rcc.cfgr.freeze();
        let mut serial = Serial::usart2(device.USART2, (tx, rx), config, clocks).unwrap();
        serial.listen(serial::Event::Rxne);
        let (serial_tx, serial_rx) = serial.split();

        // Set up the serial interface command buffer.
        let buffer = Vec::new();

        init::LateResources {
            button,
            buffer,
            exti,
            led_ring,
            serial_tx,
            serial_rx,
        }
    }

    #[task(schedule = [switch_leds], resources = [led_ring])]
    fn switch_leds() {
        resources.led_ring.lock(|led_ring| {
            if led_ring.is_mode_cycle() {
                led_ring.advance();
                schedule
                    .switch_leds(scheduled + LedRing::PERIOD.cycles())
                    .unwrap();
            }
        });
    }

    #[interrupt(binds = EXTI0, resources = [button, exti, led_cycle, serial_tx])]
    fn button_pressed() {
        resources.led_ring.lock(|led_ring| led_ring.reverse());

        // Write the fact that the button has been pressed to the serial port.
        resources
            .serial_tx
            .lock(|serial_tx| writeln!(serial_tx, "button\r").unwrap());

        resources.button.clear_interrupt_pending_bit(resources.exti);
    }

    #[interrupt(
        binds = USART2,
        priority = 2,
        resources = [buffer, led_ring, serial_rx, serial_tx],
        spawn = [switch_leds]
    )]
    fn handle_serial() {
        let buffer = resources.buffer;

        // Read a byte from the serial port and write it back.
        let byte = resources.serial_rx.read().unwrap();
        block!(resources.serial_tx.write(byte)).unwrap();
        //hprintln!("serial: {}", byte).unwrap();

        // Handle the command in the buffer for newline or backspace, otherwise append to the
        // buffer.
        if byte == b'\r' {
            block!(resources.serial_tx.write(b'\n')).unwrap();
            match &buffer[..] {
                b"flip" => {
                    resources.led_ring.reverse();
                }
                b"stop" => {
                    resources.led_ring.disable();
                }
                b"cycle" => {
                    resources.led_ring.enable_cycle();
                    spawn.switch_leds().unwrap();
                }
                b"off" => {
                    resources.led_ring.disable();
                    resources.led_ring.all_off();
                }
                b"on" => {
                    resources.led_ring.disable();
                    resources.led_ring.all_on();
                }
                _ => {
                    writeln!(resources.serial_tx, "?\r").unwrap();
                }
            }

            buffer.clear();
        } else if byte == 0x7F {
            buffer.pop();
            block!(resources.serial_tx.write(b'\r')).unwrap();
            for byte in buffer {
                block!(resources.serial_tx.write(*byte)).unwrap();
            }
        } else {
            if buffer.push(byte).is_err() {
                hprintln!("Serial read buffer full!").unwrap();
            }
        }
        //hprintln!("buffer: {:?}", buffer).unwrap();
    }

    extern "C" {
        fn TIM2();
    }
};
