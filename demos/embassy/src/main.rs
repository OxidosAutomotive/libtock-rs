#![no_std]
#![no_main]

use crate::bmp280::Control;
use core::{fmt::Write, pin::pin};
use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use embedded_hal_async::digital::Wait;
use libtock::{
    console::{ConsoleAsync, ConsoleBufWriter},
    gpio::{Gpio, PullUp},
    i2c_master::AsyncI2cMaster,
};

mod bmp280;

#[embassy_executor::task(pool_size = 4)]
async fn print(label: &'static str, period_ms: u64) {
    let mut prev;

    loop {
        prev = Instant::now();
        Timer::after_millis(period_ms).await;

        let now = Instant::now();
        let elapsed = now.duration_since(prev).as_millis();

        let mut buffer: ConsoleBufWriter<128> = ConsoleBufWriter::new();
        writeln!(
            buffer,
            "[{}] expected: {:>5}ms actual: {:>5}ms --- now: {:>10}ms",
            label,
            period_ms,
            elapsed,
            now.as_millis()
        )
        .expect("String capacity exceeded");

        ConsoleAsync::write(&mut pin!(buffer.into_allow_ro_buffer()))
            .await
            .unwrap();

        if elapsed - period_ms > 100 {
            panic!("Timer misalignment");
        }
    }
}

#[embassy_executor::task(pool_size = 4)]
async fn button(btn: u32, label: &'static str) {
    let res = Gpio::get_pin(btn);

    let Ok(pin) = res else {
        let mut buffer: ConsoleBufWriter<64> = ConsoleBufWriter::new();
        writeln!(buffer, "Pin {} does not exist.", btn).expect("String capacity exceeded");
        ConsoleAsync::write(&mut pin!(buffer.into_allow_ro_buffer()))
            .await
            .unwrap();
        return;
    };

    let mut input_pin = pin.make_input::<PullUp>().unwrap();

    loop {
        let mut buffer: ConsoleBufWriter<64> = ConsoleBufWriter::new();
        input_pin.wait_for_falling_edge().await.unwrap();

        for _ in 0..1_000_000 {}

        writeln!(buffer, "[Button] {} pressed.", label).expect("String capacity exceeded");
        ConsoleAsync::write(&mut pin!(buffer.into_allow_ro_buffer()))
            .await
            .unwrap();
    }
}

#[embassy_executor::main(stack_size = 0x3000)]
async fn main(spawner: Spawner) {
    spawner.must_spawn(print("Tick", 1000));
    spawner.must_spawn(button(12, "A"));
    spawner.must_spawn(button(13, "B"));
    spawner.must_spawn(button(14, "X"));
    spawner.must_spawn(button(15, "Y"));

    let mut bmp280 = bmp280::BMP280::new(AsyncI2cMaster);
    bmp280.read_calibration().await;
    bmp280
        .set_control(Control {
            osrs_t: bmp280::Oversampling::x2,
            osrs_p: bmp280::Oversampling::skipped,
            mode: bmp280::PowerMode::Normal,
        })
        .await;

    loop {
        let mut buffer: ConsoleBufWriter<64> = ConsoleBufWriter::new();
        let temp = bmp280.temp().await;
        writeln!(buffer, "[Temperature] {:.2}°C", temp).expect("String capacity exceeded");
        ConsoleAsync::write(&mut pin!(buffer.into_allow_ro_buffer()))
            .await
            .unwrap();
        Timer::after_secs(3).await;
    }
}
