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
use hal::gpio::{Alternate, Edge, ExtiPin, Floating, Input, Output, PushPull, AF5};
use hal::prelude::*;
use hal::serial::{self, config::Config as SerialConfig, Serial};
use hal::spi::{Mode, Phase, Polarity, Spi};
use hal::stm32::{EXTI, SPI1, USART2};
use heapless::consts::U8;
use heapless::Vec;
use rtfm::app;

type Accelerometer = hal::spi::Spi<SPI1, (Spi1Sck, Spi1Miso, Spi1Mosi)>;
type AccelerometerCs = hal::gpio::gpioe::PE3<Output<PushPull>>;
type SerialTx = hal::serial::Tx<USART2>;
type SerialRx = hal::serial::Rx<USART2>;
type Spi1Sck = hal::gpio::gpioa::PA5<Alternate<AF5>>;
type Spi1Miso = hal::gpio::gpioa::PA6<Alternate<AF5>>;
type Spi1Mosi = hal::gpio::gpioa::PA7<Alternate<AF5>>;
type UserButton = hal::gpio::gpioa::PA0<Input<Floating>>;

#[app(device = hal::stm32)]
const APP: () = {
    static mut button: UserButton = ();
    static mut buffer: Vec<u8, U8> = ();
    static mut led_ring: LedRing = ();
    static mut exti: EXTI = ();
    static mut serial_rx: SerialRx = ();
    static mut serial_tx: SerialTx = ();
    static mut accel: Accelerometer = ();
    static mut accel_cs: AccelerometerCs = ();

    #[init(spawn = [accel_leds, cycle_leds])]
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
        if led_ring.is_mode_cycle() {
            spawn.cycle_leds().unwrap();
        } else if led_ring.is_mode_accel() {
            spawn.accel_leds().unwrap();
        }

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
        let (mut serial_tx, serial_rx) = serial.split();

        // Set up the serial interface command buffer.
        let buffer = Vec::new();

        // Set up the accelerometer.
        let sck = gpioa.pa5.into_alternate_af5();
        let miso = gpioa.pa6.into_alternate_af5();
        let mosi = gpioa.pa7.into_alternate_af5();
        let mode = Mode {
            polarity: Polarity::IdleHigh,
            phase: Phase::CaptureOnSecondTransition,
        };
        let mut accel = Spi::spi1(device.SPI1, (sck, miso, mosi), mode, 100.hz(), clocks);

        let gpioe = device.GPIOE.split();
        let mut accel_cs = gpioe.pe3.into_push_pull_output();

        // Initialize the accelerometer.
        accel_cs.set_low();
        let _ = accel.transfer(&mut [0x20, 0b01000111]).unwrap();
        accel_cs.set_high();

        // Output to the serial interface that initialisation is finished.
        writeln!(serial_tx, "init\r").unwrap();

        init::LateResources {
            button,
            buffer,
            exti,
            led_ring,
            serial_tx,
            serial_rx,
            accel,
            accel_cs,
        }
    }

    #[task(schedule = [cycle_leds], resources = [led_ring])]
    fn cycle_leds() {
        resources.led_ring.lock(|led_ring| {
            if led_ring.is_mode_cycle() {
                led_ring.advance();
                schedule
                    .cycle_leds(scheduled + LedRing::PERIOD.cycles())
                    .unwrap();
            }
        });
    }

    #[task(schedule = [accel_leds], resources = [accel, accel_cs, led_ring, serial_tx])]
    fn accel_leds() {
        resources.accel_cs.set_low();
        let read_command = (1 << 7) | (1 << 6) | 0x29;
        let mut commands = [read_command, 0x0, 0x0, 0x0];
        let result = resources.accel.transfer(&mut commands[..]).unwrap();
        let acc_x = result[1] as i8;
        let acc_y = result[3] as i8;
        resources.accel_cs.set_high();

        if acc_x == 0 && acc_y == 0 {
            resources
                .serial_tx
                .lock(|serial_tx|
                    writeln!(serial_tx, "level\r").unwrap()
                );
        }

        resources.led_ring.lock(|led_ring| {
            if led_ring.is_mode_accel() {
                let directions = [acc_y < 0, acc_x < 0, acc_y > 0, acc_x > 0];
                led_ring.set_directions(directions);
                schedule.accel_leds(scheduled + LedRing::PERIOD.cycles()).unwrap();
            }
        })
    }

    #[interrupt(binds = EXTI0, resources = [button, exti, led_ring, serial_tx])]
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
        spawn = [accel_leds, cycle_leds]
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
                    spawn.cycle_leds().unwrap();
                }
                b"accel" => {
                    resources.led_ring.enable_accel();
                    spawn.accel_leds().unwrap();
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
        fn TIM3();
    }
};
