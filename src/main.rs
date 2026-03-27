#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use cortex_m_rt::entry;
use stm32f4xx_hal::{pac, prelude::*, timer::{Channel, Channel1, pwm::PwmExt}};

const AS3: u32 = 233;
const B3: u32  = 247;
const C4: u32  = 262;
const D4: u32  = 294;
const F4: u32  = 349;
const G4: u32  = 392;
const GS4: u32 = 415;
const A4: u32  = 440;
const AS4: u32 = 466;
const B4: u32  = 494;
const C5: u32  = 523;
const D5: u32  = 587;
const DS5: u32 = 622;
const E5: u32  = 659;
const F5: u32  = 698;
const REST: u32 = 0;

/*
5|--d---------------d-------|
4|dd--a--G-g-f-dfgcc--a--G-g|
1
5|--------d---------------d-|
4|-f-dfg----a--G-g-f-dfg----|
3|------bb--------------AA--|
2
5|--------------d-----------|
4|a--G-g-f-dfg*/
// (frequency_hz, duration_ms)
// 16th note = 125ms, 8th = 250ms, dotted 8th = 375ms @ 120 BPM
static MELODY: &[(u32, u32)] = &[
    // Bar 1: D D D(high) rest A rest Ab rest G F G
    (D4, 125), (D4, 125), (D5, 250), (A4, 250),
    (REST, 125), (GS4, 125), (REST, 125), (G4, 125), (REST, 125),
    (F4, 125), (REST, 125), (D4, 125), (F4, 125), (G4, 125),

    // Bar 2 (same as bar 1)
    (C4, 125), (C4, 125), (D5, 250), (A4, 250),
    (REST, 125), (GS4, 125), (REST, 125), (G4, 125), (REST, 125),
    (F4, 125), (REST, 125), (D4, 125), (F4, 125), (G4, 125),

    // Bar 3: C C D(high) rest A rest Ab rest G F G
    (B3, 125), (B3, 125), (D5, 250), (A4, 250),
    (REST, 125), (GS4, 125), (REST, 125), (G4, 125), (REST, 125),
    (F4, 125), (REST, 125), (D4, 125), (F4, 125), (G4, 125),

    // Bar 4: B4 B4 D5 rest A rest Ab rest G F G
    (AS3, 125), (AS3, 125), (D5, 250), (A4, 250),
    (REST, 125), (GS4, 125), (REST, 125), (G4, 125), (REST, 125),
    (F4, 125), (REST, 125), (D4, 125), (F4, 125), (G4, 125),
];

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr
      .use_hse(25.MHz())
      .sysclk(96.MHz())
      .freeze();

    let gpioa = dp.GPIOA.split();
    let gpioc = dp.GPIOC.split();
    let mut led = gpioc.pc13.into_push_pull_output();

    let buzzer_pin = gpioa.pa8.into_alternate::<1>();
    let channel = Channel1::new(buzzer_pin);

    // Init at an arbitrary frequency; we'll change it per note
    let mut pwm = dp.TIM1.pwm_hz(channel, 440.Hz(), &clocks);

    let mut delay = cp.SYST.delay(&clocks);

    defmt::info!("Megalovania start");

    loop {
        for &(freq, dur_ms) in MELODY.iter() {
            if freq == 0 {
                pwm.disable(Channel::C1);
                led.set_high();
            } else {
                // Reconfigure timer frequency
                pwm.set_period((freq).Hz());
                let max_duty = pwm.get_max_duty();
                pwm.set_duty(Channel::C1, max_duty / 2);
                pwm.enable(Channel::C1);
                led.set_low();
            }
            delay.delay_ms(dur_ms);
        }
        // Brief gap before looping
        pwm.disable(Channel::C1);
        led.set_high();
        delay.delay_ms(500u32);
    }
}