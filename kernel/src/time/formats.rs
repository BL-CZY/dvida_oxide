use crate::time::RtcDateTime;

// Helper function to convert RtcDateTime to POSIX timestamp
pub fn rtc_to_posix(rtc: &RtcDateTime) -> u32 {
    // Simple conversion (doesn't account for leap seconds, but good enough for ext2)
    let mut year = rtc.year as i32;
    let mut month = rtc.month as i32;

    // Adjust for months (Jan = 0, Feb = 1, etc.)
    if month <= 2 {
        year -= 1;
        month += 12;
    }

    let days =
        365 * year + year / 4 - year / 100 + year / 400 + (153 * month - 457) / 5 + rtc.day as i32
            - 719528; // Days since Unix epoch

    

    days as u32 * 86400 + rtc.hour as u32 * 3600 + rtc.minute as u32 * 60 + rtc.second as u32
}
