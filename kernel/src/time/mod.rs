use terminal::log;
use x86_64::instructions::port::Port;

pub mod formats;

/// CMOS/RTC register addresses
const RTC_SECONDS: u8 = 0x00;
const RTC_MINUTES: u8 = 0x02;
const RTC_HOURS: u8 = 0x04;
const RTC_WEEKDAY: u8 = 0x06;
const RTC_DAY: u8 = 0x07;
const RTC_MONTH: u8 = 0x08;
const RTC_YEAR: u8 = 0x09;
const RTC_CENTURY: u8 = 0x32;
const RTC_STATUS_A: u8 = 0x0A;
const RTC_STATUS_B: u8 = 0x0B;

/// RTC Status Register B flags
const RTC_24_HOUR: u8 = 0x02;
const RTC_BINARY: u8 = 0x04;
const RTC_SET_BIT: u8 = 0x80;

/// RTC Status Register A flags
const RTC_UIP: u8 = 0x80;

/// NMI disable bit
const NMI_DISABLE: u8 = 0x80;

/// Date and time structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcDateTime {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
    pub weekday: u8,
}

/// RTC Driver
pub struct Rtc {
    address_port: Port<u8>,
    data_port: Port<u8>,
}

impl Rtc {
    /// Create a new RTC driver instance
    pub const fn new() -> Self {
        Self {
            address_port: Port::new(0x70),
            data_port: Port::new(0x71),
        }
    }

    /// Read a CMOS register
    fn read_register(&mut self, reg: u8) -> u8 {
        unsafe {
            // Set NMI disable bit (bit 7) while reading
            self.address_port.write(NMI_DISABLE | reg);
            self.data_port.read()
        }
    }

    /// Write to a CMOS register
    fn write_register(&mut self, reg: u8, value: u8) {
        unsafe {
            self.address_port.write(NMI_DISABLE | reg);
            self.data_port.write(value);
        }
    }

    /// Check if an update is in progress
    unsafe fn is_update_in_progress(&mut self) -> bool {
        self.read_register(RTC_STATUS_A) & RTC_UIP != 0
    }

    /// Convert BCD to binary
    fn bcd_to_binary(bcd: u8) -> u8 {
        ((bcd >> 4) * 10) + (bcd & 0x0F)
    }

    /// Read the current date and time from RTC
    /// Returns None if the RTC is updating or on read error
    pub fn read_datetime(&mut self) -> Option<RtcDateTime> {
        // Wait for any update in progress to complete
        unsafe {
            while self.is_update_in_progress() {
                core::hint::spin_loop();
            }
        }

        // Read all values
        let second = self.read_register(RTC_SECONDS);
        let minute = self.read_register(RTC_MINUTES);
        let hour = self.read_register(RTC_HOURS);
        let day = self.read_register(RTC_DAY);
        let month = self.read_register(RTC_MONTH);
        let year = self.read_register(RTC_YEAR);
        let weekday = self.read_register(RTC_WEEKDAY);
        let century = self.read_register(RTC_CENTURY);

        // Check if another update started during our read
        unsafe {
            if self.is_update_in_progress() {
                log!("RTC update in progress during read, retrying...");
                return None;
            }
        }

        // Read status register B to check format
        let status_b = self.read_register(RTC_STATUS_B);
        let is_binary = status_b & RTC_BINARY != 0;
        let is_24hour = status_b & RTC_24_HOUR != 0;

        log!(
            "RTC format: {} mode, {} hour",
            if is_binary { "binary" } else { "BCD" },
            if is_24hour { "24" } else { "12" }
        );

        // Convert from BCD if necessary
        let second = if is_binary {
            second
        } else {
            Self::bcd_to_binary(second)
        };
        let minute = if is_binary {
            minute
        } else {
            Self::bcd_to_binary(minute)
        };
        let mut hour = if is_binary {
            hour
        } else {
            Self::bcd_to_binary(hour & 0x7F)
        };
        let day = if is_binary {
            day
        } else {
            Self::bcd_to_binary(day)
        };
        let month = if is_binary {
            month
        } else {
            Self::bcd_to_binary(month)
        };
        let year = if is_binary {
            year
        } else {
            Self::bcd_to_binary(year)
        };
        let century = if is_binary {
            century
        } else {
            Self::bcd_to_binary(century)
        };

        // Handle 12-hour format (convert to 24-hour)
        if !is_24hour {
            let pm = hour & 0x80 != 0;
            hour = hour & 0x7F;
            if pm && hour != 12 {
                hour += 12;
            } else if !pm && hour == 12 {
                hour = 0;
            }
        }

        // Calculate full year (century * 100 + year)
        // If century register is 0 or invalid, assume 20xx for years < 80, else 19xx
        let full_year = if century > 0 && century < 99 {
            (century as u16) * 100 + (year as u16)
        } else {
            if year < 80 {
                2000 + year as u16
            } else {
                1900 + year as u16
            }
        };

        log!(
            "RTC read: {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            full_year,
            month,
            day,
            hour,
            minute,
            second
        );

        Some(RtcDateTime {
            second,
            minute,
            hour,
            day,
            month,
            year: full_year,
            weekday,
        })
    }

    /// Read datetime with retry logic
    pub unsafe fn read_datetime_reliable(&mut self) -> RtcDateTime {
        log!("Reading RTC datetime...");

        // Try up to 5 times to get a consistent reading
        for attempt in 1..=5 {
            if let Some(dt) = self.read_datetime() {
                log!("RTC read successful on attempt {}", attempt);
                return dt;
            }
            log!("RTC read failed, attempt {}/5", attempt);
        }

        // Fallback - should rarely happen
        log!("ERROR: Failed to read RTC after 5 attempts!");
        panic!("Failed to read RTC after multiple attempts");
    }

    /// Convert RTC datetime to Unix timestamp (seconds since 1970-01-01 00:00:00 UTC)
    pub fn datetime_to_unix_timestamp(dt: &RtcDateTime) -> i64 {
        // Days in each month (non-leap year)
        const DAYS_IN_MONTH: [u16; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

        let year = dt.year as i64;
        let month = dt.month as i64;
        let day = dt.day as i64;

        // Calculate days since epoch (1970-01-01)
        let mut days: i64 = 0;

        // Add days for complete years
        for y in 1970..year {
            days += if Self::is_leap_year(y as u16) {
                366
            } else {
                365
            };
        }

        // Add days for complete months in current year
        for m in 1..month {
            days += DAYS_IN_MONTH[(m - 1) as usize] as i64;
            // Add leap day if February and leap year
            if m == 2 && Self::is_leap_year(year as u16) {
                days += 1;
            }
        }

        // Add remaining days
        days += day - 1;

        // Convert to seconds
        let seconds =
            days * 86400 + (dt.hour as i64) * 3600 + (dt.minute as i64) * 60 + (dt.second as i64);

        log!(
            "Converted {:04}-{:02}-{:02} {:02}:{:02}:{:02} to Unix timestamp: {}",
            dt.year,
            dt.month,
            dt.day,
            dt.hour,
            dt.minute,
            dt.second,
            seconds
        );

        seconds
    }

    /// Check if a year is a leap year
    fn is_leap_year(year: u16) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Convert Unix timestamp to RTC datetime
    pub fn unix_timestamp_to_datetime(timestamp: i64) -> RtcDateTime {
        const DAYS_IN_MONTH: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        const SECONDS_PER_DAY: i64 = 86400;

        let mut days = timestamp / SECONDS_PER_DAY;
        let mut remaining = timestamp % SECONDS_PER_DAY;

        let hour = (remaining / 3600) as u8;
        remaining %= 3600;
        let minute = (remaining / 60) as u8;
        let second = (remaining % 60) as u8;

        // Calculate year
        let mut year = 1970u16;
        loop {
            let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
            if days < days_in_year {
                break;
            }
            days -= days_in_year;
            year += 1;
        }

        // Calculate month and day
        let mut month = 1u8;
        for m in 0..12 {
            let mut days_in_month = DAYS_IN_MONTH[m] as i64;
            if m == 1 && Self::is_leap_year(year) {
                days_in_month += 1;
            }
            if days < days_in_month {
                break;
            }
            days -= days_in_month;
            month += 1;
        }
        let day = (days + 1) as u8;

        // Calculate weekday (using Zeller's congruence)
        let weekday = Self::calculate_weekday(year, month, day);

        RtcDateTime {
            second,
            minute,
            hour,
            day,
            month,
            year,
            weekday,
        }
    }

    /// Calculate day of week (0 = Sunday, 1 = Monday, etc.)
    fn calculate_weekday(year: u16, month: u8, day: u8) -> u8 {
        let mut y = year as i32;
        let mut m = month as i32;

        if m < 3 {
            m += 12;
            y -= 1;
        }

        let k = y % 100;
        let j = y / 100;

        let h = (day as i32 + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
        ((h + 6) % 7) as u8 // Convert to 0=Sunday format
    }
}
