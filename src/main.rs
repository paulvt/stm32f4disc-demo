//! STM32F4DISCOVERY demo application
//!
//! This demo application sports a serial command-interface for controlling what the LED
//! ring does: cycle clock-wise, counter clock-wise, or follow the accelerometer.

#![deny(unsafe_code)]
#![no_main]
#![no_std]

use core::fmt::Write;

use cortex_m_semihosting::hprintln;
use hal::{
    block,
    gpio::{Alternate, Edge, ExtiPin, Floating, Input, Output, PushPull, AF5},
    prelude::*,
    serial::{self, config::Config as SerialConfig, Serial},
    spi::{Mode, Phase, Polarity, Spi},
    stm32::{EXTI, SPI1, USART2},
};
use heapless::{consts::U8, Vec};
#[cfg(not(test))]
use panic_semihosting as _;
use rtfm::app;
use stm32f4disc_demo::led_ring::LedRing;

type Accelerometer = hal::spi::Spi<SPI1, (Spi1Sck, Spi1Miso, Spi1Mosi)>;
type AccelerometerCs = hal::gpio::gpioe::PE3<Output<PushPull>>;
type Led = hal::gpio::gpiod::PD<Output<PushPull>>;
type SerialTx = hal::serial::Tx<USART2>;
type SerialRx = hal::serial::Rx<USART2>;
type Spi1Sck = hal::gpio::gpioa::PA5<Alternate<AF5>>;
type Spi1Miso = hal::gpio::gpioa::PA6<Alternate<AF5>>;
type Spi1Mosi = hal::gpio::gpioa::PA7<Alternate<AF5>>;
type UserButton = hal::gpio::gpioa::PA0<Input<Floating>>;

/// The number of cycles between LED ring updates (used by tasks).
const PERIOD: u32 = 8_000_000;

#[app(device = hal::stm32)]
const APP: () = {
    static mut ACCEL: Accelerometer = ();
    static mut ACCEL_CS: AccelerometerCs = ();
    static mut BUFFER: Vec<u8, U8> = ();
    static mut BUTTON: UserButton = ();
    static mut EXTI_CNTR: EXTI = ();
    static mut LED_RING: LedRing<Led> = ();
    static mut SERIAL_RX: SerialRx = ();
    static mut SERIAL_TX: SerialTx = ();

    /// Initializes the application by setting up the LED ring, user button, serial
    /// interface and accelerometer.
    #[init(spawn = [accel_leds, cycle_leds])]
    fn init() -> init::LateResources {
        // Set up the LED ring and spawn the task corresponding to the mode.
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
        let mut exti_cntr = device.EXTI;
        let gpioa = device.GPIOA.split();
        let mut button = gpioa.pa0.into_floating_input();
        button.enable_interrupt(&mut exti_cntr);
        button.trigger_on_edge(&mut exti_cntr, Edge::RISING);

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
            ACCEL: accel,
            ACCEL_CS: accel_cs,
            BUFFER: buffer,
            BUTTON: button,
            EXTI_CNTR: exti_cntr,
            LED_RING: led_ring,
            SERIAL_RX: serial_rx,
            SERIAL_TX: serial_tx,
        }
    }

    /// Task that advances the LED ring one step and schedules the next trigger (if enabled).
    #[task(schedule = [cycle_leds], resources = [LED_RING])]
    fn cycle_leds() {
        resources.LED_RING.lock(|led_ring| {
            if led_ring.is_mode_cycle() {
                led_ring.advance();
                schedule.cycle_leds(scheduled + PERIOD.cycles()).unwrap();
            }
        });
    }

    /// Task that performs an accelerometers measurement and adjusts the LED ring accordingly
    /// and schedules the next trigger (if enabled).
    #[task(schedule = [accel_leds], resources = [ACCEL, ACCEL_CS, LED_RING, SERIAL_TX])]
    fn accel_leds() {
        resources.ACCEL_CS.set_low();
        let read_command = (1 << 7) | (1 << 6) | 0x29;
        let mut commands = [read_command, 0x0, 0x0, 0x0];
        let result = resources.ACCEL.transfer(&mut commands[..]).unwrap();
        let acc_x = result[1] as i8;
        let acc_y = result[3] as i8;
        resources.ACCEL_CS.set_high();

        if acc_x == 0 && acc_y == 0 {
            resources
                .SERIAL_TX
                .lock(|serial_tx| writeln!(serial_tx, "level\r").unwrap());
        }

        resources.LED_RING.lock(|led_ring| {
            if led_ring.is_mode_accel() {
                let directions = [acc_y < 0, acc_x < 0, acc_y > 0, acc_x > 0];
                led_ring.specific_on(directions);
                schedule.accel_leds(scheduled + PERIOD.cycles()).unwrap();
            }
        })
    }

    /// Interrupt handler that writes that the button is pressed to the serial interface
    /// and reverses the LED ring cycle direction.
    #[interrupt(binds = EXTI0, resources = [BUTTON, EXTI_CNTR, LED_RING, SERIAL_TX])]
    fn button_pressed() {
        resources.LED_RING.lock(|led_ring| led_ring.reverse());

        // Write the fact that the button has been pressed to the serial port.
        resources
            .SERIAL_TX
            .lock(|serial_tx| writeln!(serial_tx, "button\r").unwrap());

        resources
            .BUTTON
            .clear_interrupt_pending_bit(resources.EXTI_CNTR);
    }

    /// Interrupt handler that reads data from the serial connection and handles commands
    /// once an appropriate command is in the buffer.
    #[interrupt(
        binds = USART2,
        priority = 2,
        resources = [BUFFER, LED_RING, SERIAL_RX, SERIAL_TX],
        spawn = [accel_leds, cycle_leds]
    )]
    fn handle_serial() {
        let buffer = resources.BUFFER;

        // Read a byte from the serial port and write it back.
        let byte = resources.SERIAL_RX.read().unwrap();
        block!(resources.SERIAL_TX.write(byte)).unwrap();
        //hprintln!("serial: {}", byte).unwrap();

        // Handle the command in the buffer for newline or backspace, otherwise append to the
        // buffer.
        if byte == b'\r' {
            block!(resources.SERIAL_TX.write(b'\n')).unwrap();
            match &buffer[..] {
                b"flip" => {
                    resources.LED_RING.reverse();
                }
                b"stop" => {
                    resources.LED_RING.disable();
                }
                b"cycle" => {
                    resources.LED_RING.enable_cycle();
                    spawn.cycle_leds().unwrap();
                }
                b"accel" => {
                    resources.LED_RING.enable_accel();
                    spawn.accel_leds().unwrap();
                }
                b"off" => {
                    resources.LED_RING.disable();
                    resources.LED_RING.all_off();
                }
                b"on" => {
                    resources.LED_RING.disable();
                    resources.LED_RING.all_on();
                }
                _ => {
                    writeln!(resources.SERIAL_TX, "?\r").unwrap();
                }
            }

            buffer.clear();
        } else if byte == 0x7F {
            buffer.pop();
            block!(resources.SERIAL_TX.write(b'\r')).unwrap();
            for byte in buffer {
                block!(resources.SERIAL_TX.write(*byte)).unwrap();
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
