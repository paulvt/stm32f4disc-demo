stm32f4disc-demo
================

This is just some STM32F4DISCOVERY demo application using Rust.
It has a simple serial command-interface for controlling (via GPIO) what the
LED ring does: cycle clock-wise, counter clock-wise, or follow the
accelerometer.  The accelerometer is accessed via SPI.

The project also shows how to do unit testing and how to use GitLab CI.

Serial interface
----------------

The serial interface is configured on USART 2 and can be accessed using, for
example, an USB-to-serial cable connected to a ground pin, and RX to PA2 and TX
to PA3.

The interface will output the following lines:

* `init` after initialization has finished
* `button` when the user button has been pressed
* `level` when the board is being held in a perfect level position (when in
   acceleration mode)

The following lines can be given as commands:

* `on` to turn all the leds on (and disable accelerometer/cycle mode)
* `off` to turn all the leds off (and disable accelerometer/cycle mode)
* `accel` to switch to accelerometer mode
* `cycle` to switch to cycle mode
* `stop` to freeze the LEDs in the current position

License
-------

[0-clause BSD license](LICENSE.md).
