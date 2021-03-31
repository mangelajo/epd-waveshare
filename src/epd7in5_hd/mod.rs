//! A simple Driver for the Waveshare 7.5" E-Ink Display (HD) via SPI
//!
//! # References
//!
//! - [Datasheet](https://www.waveshare.com/w/upload/2/27/7inch_HD_e-Paper_Specification.pdf)
//! - [Waveshare Python driver](https://github.com/waveshare/e-Paper/blob/master/RaspberryPi_JetsonNano/python/lib/waveshare_epd/epd7in5_HD.py)
//!
use embedded_hal::{
    blocking::{delay::*, spi::Write},
    digital::v2::{InputPin, OutputPin},
};

use crate::color::Color;
use crate::interface::DisplayInterface;
use crate::traits::{InternalWiAdditions, RefreshLUT, WaveshareDisplay};

pub(crate) mod command;
use self::command::Command;

#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use self::graphics::Display7in5;

/// Width of the display
pub const WIDTH: u32 = 880;
/// Height of the display
pub const HEIGHT: u32 = 528;
/// Default Background Color
pub const DEFAULT_BACKGROUND_COLOR: Color = Color::Black; // Inverted for HD (0xFF = White)
const IS_BUSY_LOW: bool = false;

/// EPD7in5 (HD) driver
///
pub struct EPD7in5<SPI, CS, BUSY, DC, RST> {
    /// Connection Interface
    interface: DisplayInterface<SPI, CS, BUSY, DC, RST>,
    /// Background Color
    color: Color,
}

impl<SPI, CS, BUSY, DC, RST> InternalWiAdditions<SPI, CS, BUSY, DC, RST>
    for EPD7in5<SPI, CS, BUSY, DC, RST>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
{
    fn init<DELAY: DelayMs<u8>>(
        &mut self,
        spi: &mut SPI,
        delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        // Reset the device
        self.interface.reset(delay, 2);

        // HD procedure as described here:
        // https://github.com/waveshare/e-Paper/blob/master/RaspberryPi_JetsonNano/python/lib/waveshare_epd/epd7in5_HD.py
        // and as per specs:
        // https://www.waveshare.com/w/upload/2/27/7inch_HD_e-Paper_Specification.pdf

        self.wait_until_idle();
        self.command(spi, Command::SW_RESET)?;
        self.wait_until_idle();

        self.cmd_with_data(spi, Command::AUTO_WRITE_RED, &[0xF7])?;
        self.wait_until_idle();
        self.cmd_with_data(spi, Command::AUTO_WRITE_BW, &[0xF7])?;
        self.wait_until_idle();

        self.cmd_with_data(spi, Command::SOFT_START, &[0xAE, 0xC7, 0xC3, 0xC0, 0x40])?;

        self.cmd_with_data(spi, Command::DRIVER_OUTPUT_CONTROL, &[0xAF, 0x02, 0x01])?;

        self.cmd_with_data(spi, Command::DATA_ENTRY, &[0x01])?;

        self.cmd_with_data(spi, Command::SET_RAM_X_START_END, &[0x00, 0x00, 0x6F, 0x03])?;
        self.cmd_with_data(spi, Command::SET_RAM_Y_START_END, &[0xAF, 0x02, 0x00, 0x00])?;

        self.cmd_with_data(spi, Command::VBD_CONTROL, &[0x05])?;

        self.cmd_with_data(spi, Command::TEMPERATURE_SENSOR_CONTROL, &[0x80])?;

        self.cmd_with_data(spi, Command::DISPLAY_UPDATE_CONTROL_2, &[0xB1])?;

        self.command(spi, Command::MASTER_ACTIVATION)?;
        self.wait_until_idle();

        self.cmd_with_data(spi, Command::SET_RAM_X_AC, &[0x00, 0x00])?;
        self.cmd_with_data(spi, Command::SET_RAM_Y_AC, &[0x00, 0x00])?;

        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST> WaveshareDisplay<SPI, CS, BUSY, DC, RST>
    for EPD7in5<SPI, CS, BUSY, DC, RST>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
{
    type DisplayColor = Color;
    fn new<DELAY: DelayMs<u8>>(
        spi: &mut SPI,
        cs: CS,
        busy: BUSY,
        dc: DC,
        rst: RST,
        delay: &mut DELAY,
    ) -> Result<Self, SPI::Error> {
        let interface = DisplayInterface::new(cs, busy, dc, rst);
        let color = DEFAULT_BACKGROUND_COLOR;

        let mut epd = EPD7in5 { interface, color };

        epd.init(spi, delay)?;

        Ok(epd)
    }

    fn wake_up<DELAY: DelayMs<u8>>(
        &mut self,
        spi: &mut SPI,
        delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        self.init(spi, delay)
    }

    fn sleep(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.cmd_with_data(spi, Command::DEEP_SLEEP, &[0x01])?;
        Ok(())
    }

    fn update_frame(&mut self, spi: &mut SPI, buffer: &[u8]) -> Result<(), SPI::Error> {
        self.wait_until_idle();
        self.cmd_with_data(spi, Command::SET_RAM_Y_AC, &[0x00, 0x00])?;
        self.cmd_with_data(spi, Command::WRITE_RAM_BW, buffer)?;
        self.cmd_with_data(spi, Command::DISPLAY_UPDATE_CONTROL_2, &[0xF7])?;
        Ok(())
    }

    fn update_partial_frame(
        &mut self,
        _spi: &mut SPI,
        _buffer: &[u8],
        _x: u32,
        _y: u32,
        _width: u32,
        _height: u32,
    ) -> Result<(), SPI::Error> {
        unimplemented!();
    }

    fn display_frame(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        self.command(spi, Command::MASTER_ACTIVATION)?;
        self.wait_until_idle();
        Ok(())
    }

    fn update_and_display_frame(&mut self, spi: &mut SPI, buffer: &[u8]) -> Result<(), SPI::Error> {
        self.update_frame(spi, buffer)?;
        self.display_frame(spi)?;
        Ok(())
    }

    fn clear_frame(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        let pixel_count = WIDTH * HEIGHT / 8;
        let blank_frame = [0xFF; (WIDTH * HEIGHT / 8) as usize];

        // self.update_and_display_frame(spi, &blank_frame)?;

        self.wait_until_idle();
        self.cmd_with_data(spi, Command::SET_RAM_Y_AC, &[0x00, 0x00])?;

        for cmd in &[Command::WRITE_RAM_BW, Command::WRITE_RAM_RED] {
            self.command(spi, *cmd)?;
            self.interface.data_x_times(spi, 0xFF, pixel_count)?;
        }

        self.cmd_with_data(spi, Command::DISPLAY_UPDATE_CONTROL_2, &[0xF7])?;
        self.command(spi, Command::MASTER_ACTIVATION)?;
        self.wait_until_idle();
        Ok(())
    }

    fn set_background_color(&mut self, color: Color) {
        self.color = color;
    }

    fn background_color(&self) -> &Color {
        &self.color
    }

    fn width(&self) -> u32 {
        WIDTH
    }

    fn height(&self) -> u32 {
        HEIGHT
    }

    fn set_lut(
        &mut self,
        _spi: &mut SPI,
        _refresh_rate: Option<RefreshLUT>,
    ) -> Result<(), SPI::Error> {
        unimplemented!();
    }

    fn is_busy(&self) -> bool {
        self.interface.is_busy(IS_BUSY_LOW)
    }
}

impl<SPI, CS, BUSY, DC, RST> EPD7in5<SPI, CS, BUSY, DC, RST>
where
    SPI: Write<u8>,
    CS: OutputPin,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
{
    fn command(&mut self, spi: &mut SPI, command: Command) -> Result<(), SPI::Error> {
        self.interface.cmd(spi, command)
    }

    fn send_data(&mut self, spi: &mut SPI, data: &[u8]) -> Result<(), SPI::Error> {
        self.interface.data(spi, data)
    }

    fn cmd_with_data(
        &mut self,
        spi: &mut SPI,
        command: Command,
        data: &[u8],
    ) -> Result<(), SPI::Error> {
        self.interface.cmd_with_data(spi, command, data)
    }

    fn wait_until_idle(&mut self) {
        self.interface.wait_until_idle(IS_BUSY_LOW)
    }

    // fn send_resolution(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
    //     unimplemented!();
    //     // let w = self.width();
    //     // let h = self.height();

    //     // self.cmd_with_data(spi, Command::SET_RAM_Y_AC, &[0x00, 0x00])?;

    //     // self.command(spi, Command::TCON_RESOLUTION)?;
    //     // self.send_data(spi, &[(w >> 8) as u8])?;
    //     // self.send_data(spi, &[w as u8])?;
    //     // self.send_data(spi, &[(h >> 8) as u8])?;
    //     // self.send_data(spi, &[h as u8])
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epd_size() {
        assert_eq!(WIDTH, 880);
        assert_eq!(HEIGHT, 528);
        assert_eq!(DEFAULT_BACKGROUND_COLOR, Color::Black);
    }
}
