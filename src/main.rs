#![no_std]
#![no_main]

mod usb_jboot;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::peripherals::USB_OTG_FS;
use embassy_stm32::time::Hertz;
use embassy_stm32::usb::Driver;
use embassy_stm32::{bind_interrupts, peripherals, usb, Config};
use embassy_usb::Builder;
use static_cell::StaticCell;
use usb_jboot::JBoot;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

// big static buffer for the USB class to use
static BIG_BUF: StaticCell<[u8; 0x500]> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("J-Link OpenBoot: Open-source bootloader for J-Link V9");

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

    // Create the driver, from the HAL.
    let mut ep_out_buffer = [0u8; 256];
    let mut config = embassy_stm32::usb::Config::default();

    // Do not enable vbus_detection. This is a safe default that works in all boards.
    // However, if your USB device is self-powered (can stay powered on if USB is unplugged), you need
    // to enable vbus_detection to comply with the USB spec. If you enable it, the board
    // has to support it or USB won't work at all. See docs on `vbus_detection` for details.
    config.vbus_detection = false;

    let driver = Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, &mut ep_out_buffer, config);

    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0x1366, 0x0101);
    config.manufacturer = Some("SEGGER");
    config.product = Some("J-Link OpenBoot");
    // config.product = Some("J-Link");
    config.serial_number = Some("000000123456");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    config.device_class = 0x0;
    config.device_sub_class = 0x0;
    config.device_protocol = 0x0;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut [], // embassy-rs PR pending to support zero length BOS descriptor
        &mut [],
        &mut control_buf,
    );

    // Add a vendor-specific function (class 0xFF), and corresponding interface,
    // that uses our custom handler.
    let mut function = builder.function(0xFF, 0, 0);
    let mut interface = function.interface();
    let mut alt = interface.alt_setting(0xFF, 0xFF, 0xFF, None);
    let write_ep = alt.endpoint_bulk_in(64);
    let read_ep = alt.endpoint_bulk_out(64);
    drop(function);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    let mut jboot= JBoot::<Driver<USB_OTG_FS>>::new(read_ep, write_ep, &BIG_BUF);

    info!("Starting JBoot USB task...");
    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, jboot.run()).await;
}
