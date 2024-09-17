#![no_std]
#![no_main]
mod usb_jboot;
mod bulk_usb;

use cortex_m::peripheral::NVIC;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::time::Hertz;
use embassy_stm32::{pac, Config, Peripherals};
use embassy_stm32::flash::{Flash};
use static_cell::StaticCell;
use usb_jboot::JBoot;
use {defmt_rtt as _, panic_probe as _};

// big static buffer for the USB class to use
static BIG_BUF: StaticCell<[u8; 0x500]> = StaticCell::new();

async fn app_crc16(start: u32, len: usize) -> u16 {
    let mut crc: u16 = 0x0;
    let addr = start;
    let len = len;

    // read 4 bytes at a time, and calculate the CRC-16
    for i in 0..(len) {
        let data = unsafe { core::ptr::read_volatile((addr + i as u32) as *const u8) };
        crc ^= data as u16;

        for _ in 0..8 {
            let bit = crc & 1;
            if bit != 0 {
                crc = (crc >> 1) ^ 0x8408; // LSB set: XOR with polynomial
            } else {
                crc >>= 1; // Shift right if LSB is not set
            }
        }
    }

    crc & 0xFFFFu16 // Return the final 16-bit CRC value
}


async fn check_for_valid_application(p: &mut Peripherals) -> bool {
    let mut flash = Flash::new_blocking(&mut p.FLASH);

    let mut buf = [0u8; 4];
    flash.blocking_read(0x1_0000, &mut buf).unwrap();
    if buf == [0x00, 0x00, 0x00, 0x00] {
        info!("[*] Vectors NOT found at 0x0801_0000, going to bootloader");
        return false;
    }
    info!("[*] Vectors found at 0x0801_0000");

    // 803ff00 must contain "www.segger.com"
    let mut buf = [0u8; 14];
    flash.blocking_read(0x3_FF00, &mut buf).unwrap();
    let segger_buf = [0x77, 0x77, 0x77, 0x2e, 0x73, 0x65, 0x67, 0x67, 0x65, 0x72, 0x2e, 0x63, 0x6f, 0x6d];
    if buf != segger_buf {
        info!("[*] Segger string NOT found at 0x0803_FF00, going to bootloader");
        return false;
    }
    info!("[*] Segger string found at 0x0803_FF00");

    // 803fffe must contain CRC-16 of the application
    let mut buf = [0u8; 2];
    flash.blocking_read(0x3_FFFE, &mut buf).unwrap();
    let crc = u16::from_le_bytes(buf);
    info!("[*] CRC-16 of the application: 0x{:04x}", crc);

    let app_crc = app_crc16(0x0801_0000, 0x2_FFFE).await;
    if crc != app_crc {
        info!("[*] CRC-16 mismatch: 0x{:04x} != 0x{:04x}, going to bootloader", crc, app_crc);
        return false;
    }
    info!("[*] CRC-16 matches");
    true
}

/// Boots the application.
///
/// # Safety
///
/// This modifies the stack pointer and reset vector and will run code placed in the active partition.
pub unsafe fn load(start: u32) -> ! {
    info!("Loading app at 0x{:x}", start);
    #[allow(unused_mut)]
    let mut p = cortex_m::Peripherals::steal();

    cortex_m::interrupt::disable();

    NVIC::mask(pac::interrupt::TIM8_BRK_TIM12);
    NVIC::unpend(pac::interrupt::TIM8_BRK_TIM12);

    p.SCB.invalidate_icache();
    p.SCB.vtor.write(start);

    cortex_m::asm::bootload(start as *const u32)
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("J-Link OpenBoot: Open-source bootloader for J-Link V9");
    let mut p = setup_peripherals();

    info!("Checking for valid application...");
    let valid = check_for_valid_application(&mut p).await;

    match valid {
        false => {
            info!("No valid application found. Starting USB task...");
            info!("Starting JBoot USB task...");
            let mut jboot = JBoot::new(&BIG_BUF, p);
            jboot.run().await;
            return;
        }
        true => unsafe {
            info!("Valid application found: {}", valid);
            load(0x0801_0000);
        }
    }
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

