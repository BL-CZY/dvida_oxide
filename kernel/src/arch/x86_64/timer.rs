use core::{
    ops::Sub,
    sync::atomic::{AtomicU32, AtomicU64},
    time::Duration,
};

use crate::{arch::x86_64::idt::GSI_TO_IRQ_MAPPING, log};
use alloc::collections::btree_map::BTreeMap;
use limine::{mp::Cpu, request::DateAtBootRequest};
use once_cell_no_std::OnceCell;
use x86_64::instructions::port::{Port, PortWriteOnly};

use crate::arch::x86_64::{acpi::apic::LocalApic, pic::PRIMARY_ISA_PIC_OFFSET};

pub const PIT_DATA_PORT: u16 = 0x40;
pub const PIT_CMD_REGISTER: u16 = 0x43;

pub static APIC_TIMER_TICKS_PER_MS: OnceCell<BTreeMap<u32, AtomicU32>> = OnceCell::new();
pub static TSC_TIMER_TICKS_PER_MS: AtomicU64 = AtomicU64::new(0);
pub static TIME_AT_BOOT: AtomicU64 = AtomicU64::new(0);

#[used]
#[unsafe(link_section = ".requests")]
pub static DATE_AT_BOOT_REQUEST: DateAtBootRequest = DateAtBootRequest::new();

pub fn configure_pit() {
    const CHANNEL_3_OSCILATOR: u8 = 0x36;
    configure_pit_with_divisor(0, CHANNEL_3_OSCILATOR);

    log!("PIT configured to have a divisor of 0")
}

pub fn configure_pit_with_divisor(divisor: u16, channel: u8) {
    let mut data_port: PortWriteOnly<u8> = PortWriteOnly::new(PIT_DATA_PORT);
    let mut cmd_port: Port<u8> = Port::new(PIT_CMD_REGISTER);

    unsafe {
        x86_64::instructions::interrupts::without_interrupts(|| {
            cmd_port.write(channel);
            data_port.write((divisor & 0xFF) as u8);
            data_port.write(((divisor >> 8) & 0xFF) as u8);
        });
    }
}

pub fn read_pit_count() -> u16 {
    let mut data_port: Port<u8> = Port::new(PIT_DATA_PORT);
    let mut cmd_port: Port<u8> = Port::new(PIT_CMD_REGISTER);

    const LATCH_CHANNEL_0: u8 = 0;

    unsafe {
        cmd_port.write(LATCH_CHANNEL_0);

        let lo: u8 = data_port.read();

        let hi: u8 = data_port.read();

        lo as u16 | (hi as u16) << 8
    }
}

pub fn get_apic_timer_ticks_per_ms(cpu_id: u32) -> &'static AtomicU32 {
    APIC_TIMER_TICKS_PER_MS
        .get()
        .expect("No array found")
        .get(&cpu_id)
        .expect("Corrupted data")
}

pub const TIMER_PERIODIC_MODE: u32 = 0x20000;

impl LocalApic {
    pub fn initialize_timer_array(cpus: &[&Cpu]) {
        let mut res = BTreeMap::new();

        for cpu in cpus.iter() {
            res.insert(cpu.id, AtomicU32::new(0));
        }

        let _ = APIC_TIMER_TICKS_PER_MS.set(res);
    }

    pub fn load_timer(&mut self, cpu_id: u32, frequency: u32) {
        let freq = get_apic_timer_ticks_per_ms(cpu_id);
        freq.store(frequency, core::sync::atomic::Ordering::Relaxed);

        let vector = GSI_TO_IRQ_MAPPING.get().expect("No mappings found")[0];

        log!(
            "{frequency} ticks have elapsed in 1 ms for APIC! Enabling the timer to vector: {:?}",
            vector + PRIMARY_ISA_PIC_OFFSET as u32
        );

        self.write_lvt_timer((vector + PRIMARY_ISA_PIC_OFFSET as u32) | TIMER_PERIODIC_MODE);
        // one every 1 micro seconds
        self.write_timer_initial_count(frequency as u32);
    }

    pub fn calibrate_timer(&mut self) {
        const DIVIDE_BY_16_CONF: u32 = 0x3;

        self.write_timer_divide_config(DIVIDE_BY_16_CONF);

        configure_pit_with_divisor(TEN_MS_DIVISOR, CHANNEL_1_COUNT_DOWN);
        self.write_timer_initial_count(u32::MAX);

        let mut ticks_elapsed;
        let init_time = self.read_timer_current_count();

        loop {
            let time = self.read_timer_current_count();
            ticks_elapsed = init_time - time;

            let count = read_pit_count();

            if count == 0 || count > TEN_MS_DIVISOR {
                break;
            }
        }

        let cpu_id = self.read_id();
        self.load_timer(cpu_id, ticks_elapsed / 10);
    }
}

const TEN_MS_DIVISOR: u16 = 11932;
const CHANNEL_1_COUNT_DOWN: u8 = 0x30;

pub fn calibrate_tsc() {
    // unixtimestamp at boot
    let date_at_boot = DATE_AT_BOOT_REQUEST
        .get_response()
        .expect("No date at boot")
        .timestamp()
        .as_secs();

    TIME_AT_BOOT.store(date_at_boot, core::sync::atomic::Ordering::Relaxed);

    configure_pit_with_divisor(TEN_MS_DIVISOR, CHANNEL_1_COUNT_DOWN);

    let init_tick_count = unsafe { core::arch::x86_64::_rdtsc() };

    loop {
        let count = read_pit_count();

        if count == 0 || count > TEN_MS_DIVISOR {
            break;
        }
    }

    let tick_count = unsafe { core::arch::x86_64::_rdtsc() } - init_tick_count;

    TSC_TIMER_TICKS_PER_MS.store(tick_count / 10, core::sync::atomic::Ordering::Relaxed);

    log!("{tick_count} ticks have elapsed in 10 ms for tsc!",);
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct Instant(u64);

impl Instant {
    pub fn now() -> Self {
        let ticks = unsafe { core::arch::x86_64::_rdtsc() };

        Self(ticks)
    }

    pub fn as_timestamp_secs(&self) -> u64 {
        let boot_time = TIME_AT_BOOT.load(core::sync::atomic::Ordering::Relaxed);
        let ticks_per_millis = TSC_TIMER_TICKS_PER_MS.load(core::sync::atomic::Ordering::Relaxed);
        let ticks_per_seconds = ticks_per_millis * 1000;

        if ticks_per_seconds == 0 {
            panic!("Function should not be called before timer initialization")
        } else {
            boot_time + self.0 / ticks_per_seconds
        }
    }

    pub fn as_timestamp_millis(&self) -> u64 {
        let boot_time_ms = TIME_AT_BOOT.load(core::sync::atomic::Ordering::Relaxed) * 1000;
        let ticks_per_millis = TSC_TIMER_TICKS_PER_MS.load(core::sync::atomic::Ordering::Relaxed);

        if ticks_per_millis == 0 {
            panic!("Function should not be called before timer initialization")
        } else {
            boot_time_ms + self.0 / ticks_per_millis
        }
    }
}

macro_rules! nanos_per_tick {
    ($ticks_per_millis:ident) => {
        1000_000_000u128 / $ticks_per_millis as u128
    };
}

// TODO: make the fs driver use this
impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, rhs: Instant) -> Self::Output {
        let ticks_per_millis = TSC_TIMER_TICKS_PER_MS.load(core::sync::atomic::Ordering::Relaxed);

        let ticks = self.0.saturating_sub(rhs.0) as u128;
        let nanos = ticks * nanos_per_tick!(ticks_per_millis);

        Duration::from_nanos_u128(nanos)
    }
}

pub fn blocking_sleep(time: Duration) {
    let instant = Instant::now();

    loop {
        if Instant::now() - instant >= time {
            return;
        }

        core::hint::spin_loop();
    }
}
