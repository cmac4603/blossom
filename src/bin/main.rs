#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use core::cell::Cell;
use core::cell::RefCell;

use bt_hci::controller::ExternalController;
use critical_section::Mutex;
use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use embedded_graphics::image::Image;
use embedded_graphics::image::ImageRaw;
use embedded_graphics::image::ImageRawLE;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_bus::spi::RefCellDevice;
// use embedded_sdmmc::SdCard;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig, Pull};
use esp_hal::spi;
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_radio::ble::controller::BleConnector;
use st7735_lcd;
use st7735_lcd::Orientation;
use trouble_host::prelude::*;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32c5 -o unstable-hal -o alloc -o wifi -o embassy -o ble-trouble -o neovim -o stable-x86_64-unknown-linux-gnu

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 1>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);

    // TODO: Spawn some tasks
    let _ = spawner;

    let spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(400))
            .with_mode(spi::Mode::_0),
    )
    .expect("should get the bus ok")
    .with_sck(peripherals.GPIO6)
    .with_mosi(peripherals.GPIO2)
    .with_miso(peripherals.GPIO7)
    .into_async();

    let bus = RefCell::new(spi_bus);

    let lcd_rs = Output::new(
        peripherals.GPIO10,
        Level::High,
        OutputConfig::default().with_pull(Pull::Up),
    );
    let lcd_dev = RefCellDevice::new(&bus, lcd_rs, Delay).unwrap();

    let rst = Output::new(
        peripherals.GPIO1,
        Level::Low,
        OutputConfig::default().with_pull(Pull::Up),
    );
    let dc = Output::new(
        peripherals.GPIO3,
        Level::Low,
        OutputConfig::default().with_pull(Pull::Up),
    );

    let rgb = false;
    let inverted = true;
    let width = 110;
    let height = 161;

    let mut display = st7735_lcd::ST7735::new(lcd_dev, dc, rst, rgb, inverted, width, height);
    display.init(&mut Delay).unwrap();
    display.clear(Rgb565::BLACK).unwrap();
    display.set_orientation(&Orientation::Landscape).unwrap();
    display.set_offset(15, 25);

    let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(include_bytes!("../assets/ferris.raw"), 86);
    let image = Image::new(&image_raw, Point::new(26, 8));
    image.draw(&mut display).unwrap();

    println!("lcd test have done.");

    loop {
        Timer::after(Duration::from_secs(10)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}
