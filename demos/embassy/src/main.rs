#![no_std]
#![no_main]

use crate::bmp280::Control;
use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use embedded_hal_async::digital::Wait;
use heapless::String;
use libtock::{
    console::ConsoleAsync,
    gpio::{Gpio, PullUp},
    i2c_master::AsyncI2cMaster,
};

mod bmp280;

#[embassy_executor::task(pool_size = 4)]
async fn print(label: &'static str, period_ms: u64) {
    let mut prev;
    let mut string: String<128> = String::new();

    loop {
        prev = Instant::now();
        Timer::after_millis(period_ms).await;

        let now = Instant::now();
        let elapsed = now.duration_since(prev).as_millis();

        string.clear();
        writeln!(
            string,
            "[{}] expected: {:>5}ms actual: {:>5}ms --- now: {:>10}ms",
            label,
            period_ms,
            elapsed,
            now.as_millis()
        )
        .expect("String capacity exceeded");

        ConsoleAsync::write(string.as_bytes()).await.unwrap();

        if elapsed - period_ms > 100 {
            panic!("Timer misalignment");
        }
    }
}

#[embassy_executor::task(pool_size = 4)]
async fn button(btn: u32, label: &'static str) {
    let res = Gpio::get_pin(btn);
    let mut string: String<64> = String::new();

    let Ok(pin) = res else {
        writeln!(string, "Pin {} does not exist.", btn).expect("String capacity exceeded");
        ConsoleAsync::write(string.as_bytes()).await.unwrap();
        return;
    };

    let mut input_pin = pin.make_input::<PullUp>().unwrap();

    loop {
        input_pin.wait_for_falling_edge().await.unwrap();
        writeln!(string, "[Button] {} pressed.", label).expect("String capacity exceeded");
        ConsoleAsync::write(string.as_bytes()).await.unwrap();
        string.clear();
    }
}

#[embassy_executor::main(stack_size = 0x3000)]
async fn main(spawner: Spawner) {
    spawner.must_spawn(print("Tick", 1000));
    spawner.must_spawn(button(12, "A"));
    spawner.must_spawn(button(13, "B"));
    spawner.must_spawn(button(14, "X"));
    spawner.must_spawn(button(15, "Y"));

    let mut bmp280 = bmp280::BMP280::new(AsyncI2cMaster).unwrap();
    bmp280.read_calibration().await;
    bmp280
        .set_control(Control {
            osrs_t: bmp280::Oversampling::x2,
            osrs_p: bmp280::Oversampling::skipped,
            mode: bmp280::PowerMode::Normal,
        })
        .await;

    let mut string: String<64> = String::new();
    loop {
        let temp = bmp280.temp().await;
        writeln!(string, "[Temperature] {:.2}°C", temp).expect("String capacity exceeded");
        ConsoleAsync::write(string.as_bytes()).await.unwrap();
        string.clear();
        Timer::after_secs(3).await;
    }
}
