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
use rtfm::cyccnt::{Instant, U32Ext};
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

#[app(device = hal::stm32, monotonic = rtfm::cyccnt::CYCCNT, peripherals = true)]
const APP: () = {
    struct Resources {
        accel: Accelerometer,
        accel_cs: AccelerometerCs,
        buffer: Vec<u8, U8>,
        button: UserButton,
        exit_cntr: EXTI,
        led_ring: LedRing<Led>,
        serial_rx: SerialRx,
        serial_tx: SerialTx,
    }

    /// Initializes the application by setting up the LED ring, user button, serial
    /// interface and accelerometer.
    #[init(spawn = [accel_leds, cycle_leds])]
    fn init(mut cx: init::Context) -> init::LateResources {
        // Set up and enable the monotonic timer.
        cx.core.DCB.enable_trace();
        cx.core.DWT.enable_cycle_counter();

        // Set up the LED ring and spawn the task corresponding to the mode.
        let gpiod = cx.device.GPIOD.split();
        let leds = [
            gpiod.pd12.into_push_pull_output().downgrade(),
            gpiod.pd13.into_push_pull_output().downgrade(),
            gpiod.pd14.into_push_pull_output().downgrade(),
            gpiod.pd15.into_push_pull_output().downgrade(),
        ];
        let led_ring = LedRing::from(leds);
        if led_ring.is_mode_cycle() {
            cx.spawn.cycle_leds().unwrap();
        } else if led_ring.is_mode_accel() {
            cx.spawn.accel_leds().unwrap();
        }

        // Set up the EXTI0 interrupt for the user button.
        let mut exti_cntr = cx.device.EXTI;
        let gpioa = cx.device.GPIOA.split();
        let mut button = gpioa.pa0.into_floating_input();
        button.enable_interrupt(&mut exti_cntr);
        button.trigger_on_edge(&mut exti_cntr, Edge::RISING);

        // Set up the serial interface and the USART2 interrupt.
        let tx = gpioa.pa2.into_alternate_af7();
        let rx = gpioa.pa3.into_alternate_af7();
        let config = SerialConfig::default().baudrate(115_200.bps());
        let rcc = cx.device.RCC.constrain();
        let clocks = rcc.cfgr.freeze();
        let mut serial = Serial::usart2(cx.device.USART2, (tx, rx), config, clocks).unwrap();
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
        let mut accel = Spi::spi1(cx.device.SPI1, (sck, miso, mosi), mode, 100.hz(), clocks);

        let gpioe = cx.device.GPIOE.split();
        let mut accel_cs = gpioe.pe3.into_push_pull_output();

        // Initialize the accelerometer.
        accel_cs.set_low().unwrap();
        let _ = accel.transfer(&mut [0x20, 0b01000111]).unwrap();
        accel_cs.set_high().unwrap();

        // Output to the serial interface that initialisation is finished.
        writeln!(serial_tx, "init\r").unwrap();

        init::LateResources {
            accel: accel,
            accel_cs: accel_cs,
            buffer: buffer,
            button: button,
            exit_cntr: exti_cntr,
            led_ring: led_ring,
            serial_rx: serial_rx,
            serial_tx: serial_tx,
        }
    }

    /// Task that advances the LED ring one step and schedules the next trigger (if enabled).
    #[task(schedule = [cycle_leds], resources = [led_ring])]
    fn cycle_leds(mut cx: cycle_leds::Context) {
        let reschedule = cx.resources.led_ring.lock(|led_ring| {
            if led_ring.is_mode_cycle() {
                led_ring.advance();
                true
            } else {
                false
            }
        });

        if reschedule {
            cx.schedule
                .cycle_leds(Instant::now() + PERIOD.cycles())
                .unwrap();
        }
    }

    /// Task that performs an accelerometers measurement and adjusts the LED ring accordingly
    /// and schedules the next trigger (if enabled).
    #[task(schedule = [accel_leds], resources = [accel, accel_cs, led_ring, serial_tx])]
    fn accel_leds(mut cx: accel_leds::Context) {
        cx.resources.accel_cs.set_low().unwrap();
        let read_command = (1 << 7) | (1 << 6) | 0x29;
        let mut commands = [read_command, 0x0, 0x0, 0x0];
        let result = cx.resources.accel.transfer(&mut commands[..]).unwrap();
        let acc_x = result[1] as i8;
        let acc_y = result[3] as i8;
        cx.resources.accel_cs.set_high().unwrap();

        if acc_x == 0 && acc_y == 0 {
            cx.resources
                .serial_tx
                .lock(|serial_tx| writeln!(serial_tx, "level\r").unwrap());
        }

        let reschedule = cx.resources.led_ring.lock(|led_ring| {
            if led_ring.is_mode_accel() {
                let directions = [acc_y < 0, acc_x < 0, acc_y > 0, acc_x > 0];
                led_ring.specific_on(directions);
                true
            } else {
                false
            }
        });

        if reschedule {
            cx.schedule
                .accel_leds(Instant::now() + PERIOD.cycles())
                .unwrap();
        }
    }

    /// Interrupt handler that writes that the button is pressed to the serial interface
    /// and reverses the LED ring cycle direction.
    #[task(binds = EXTI0, resources = [button, exit_cntr, led_ring, serial_tx])]
    fn button_pressed(mut cx: button_pressed::Context) {
        cx.resources.led_ring.lock(|led_ring| led_ring.reverse());

        // Write the fact that the button has been pressed to the serial port.
        cx.resources
            .serial_tx
            .lock(|serial_tx| writeln!(serial_tx, "button\r").unwrap());

        cx.resources
            .button
            .clear_interrupt_pending_bit(cx.resources.exit_cntr);
    }

    /// Interrupt handler that reads data from the serial connection and handles commands
    /// once an appropriate command is in the buffer.
    #[task(
        binds = USART2,
        priority = 2,
        resources = [buffer, led_ring, serial_rx, serial_tx],
        spawn = [accel_leds, cycle_leds]
    )]
    fn handle_serial(cx: handle_serial::Context) {
        let buffer = cx.resources.buffer;

        // Read a byte from the serial port and write it back.
        let byte = cx.resources.serial_rx.read().unwrap();
        block!(cx.resources.serial_tx.write(byte)).unwrap();
        //hprintln!("serial: {}", byte).unwrap();

        // Handle the command in the buffer for newline or backspace, otherwise append to the
        // buffer.
        if byte == b'\r' {
            block!(cx.resources.serial_tx.write(b'\n')).unwrap();
            match &buffer[..] {
                b"flip" => {
                    cx.resources.led_ring.reverse();
                }
                b"stop" => {
                    cx.resources.led_ring.disable();
                }
                b"cycle" => {
                    cx.resources.led_ring.enable_cycle();
                    cx.spawn.cycle_leds().unwrap();
                }
                b"accel" => {
                    cx.resources.led_ring.enable_accel();
                    cx.spawn.accel_leds().unwrap();
                }
                b"off" => {
                    cx.resources.led_ring.disable();
                    cx.resources.led_ring.all_off();
                }
                b"on" => {
                    cx.resources.led_ring.disable();
                    cx.resources.led_ring.all_on();
                }
                _ => {
                    writeln!(cx.resources.serial_tx, "?\r").unwrap();
                }
            }

            buffer.clear();
        } else if byte == 0x7F {
            buffer.pop();
            block!(cx.resources.serial_tx.write(b'\r')).unwrap();
            for byte in buffer {
                block!(cx.resources.serial_tx.write(*byte)).unwrap();
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
