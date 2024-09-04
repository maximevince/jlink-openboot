#![no_std]
#![no_main]

mod usb_jboot;
mod bulk_usb;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_stm32::flash::{Blocking, Flash};
use static_cell::StaticCell;
use usb_jboot::JBoot;
use {defmt_rtt as _, panic_probe as _};

// big static buffer for the USB class to use
static BIG_BUF: StaticCell<[u8; 0x500]> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("J-Link OpenBoot: Open-source bootloader for J-Link V9");

    let p = setup_peripherals();

    info!("Starting JBoot USB task...");
    let mut jboot = JBoot::new(&BIG_BUF, p);
    jboot.run().await;
}

fn setup_peripherals() -> embassy_stm32::Peripherals {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(12_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            // 12 MHz clock source / 12 = 1 MHz PLL input
            prediv: unwrap!(PllPreDiv::try_from(12)),
            // 1 MHz PLL input * 240 = 240 MHz PLL VCO
            mul: unwrap!(PllMul::try_from(240)),
            // 240 MHz PLL VCO / 2 = 120 MHz main PLL output
            divp: Some(PllPDiv::DIV2),
            // 240 MHz PLL VCO / 5 = 48 MHz PLL48 output
            divq: Some(PllQDiv::DIV5),
            divr: None,
        });
        // System clock comes from PLL (= the 120 MHz main PLL output)
        config.rcc.sys = Sysclk::PLL1_P;
        // 120 MHz / 4 = 30 MHz APB1 frequency
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        // 120 MHz / 2 = 60 MHz APB2 frequency
        config.rcc.apb2_pre = APBPrescaler::DIV2;
    }
    let p = embassy_stm32::init(config);
    p
}

