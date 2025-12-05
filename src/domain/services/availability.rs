use chrono::{NaiveDate, NaiveTime, Datelike, Timelike, Utc, Duration, TimeZone};
use chrono_tz::Tz;
use crate::domain::models::event::{Event, WeekdayConfig};
use crate::domain::models::booking::Booking;
use crate::domain::models::event_override::EventOverride;
use crate::domain::models::session::EventSession;
use std::cmp::{max, min};

const TOTAL_MINUTES: usize = 1440;

pub fn calculate_slots(
    event: &Event,
    date: NaiveDate,
    existing_bookings: &[Booking],
    override_rule: Option<&EventOverride>,
    manual_sessions: Option<&[EventSession]>
) -> Vec<String> {
    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);

    if event.schedule_type == "MANUAL" {
        let mut valid_slots = Vec::new();
        if let Some(sessions) = manual_sessions {
            for session in sessions {
                let session_start_tz = session.start_time.with_timezone(&tz);
                if session_start_tz.date_naive() != date {
                    continue;
                }

                let booking_count = existing_bookings.iter().filter(|b| {
                    b.start_time < session.end_time && b.end_time > session.start_time
                }).count();

                if (booking_count as i32) < session.max_participants {
                    valid_slots.push(session.start_time.to_rfc3339());
                }
            }
        }
        valid_slots.sort();
        valid_slots.dedup();
        return valid_slots;
    }

    if override_rule.is_some_and(|r| r.is_unavailable) {
        return Vec::new();
    }

    let config: WeekdayConfig = if let Some(rule) = override_rule {
        if let Some(ref json) = rule.override_config_json {
            serde_json::from_str(json).unwrap_or_else(|_|
                serde_json::from_str(&event.config_json).unwrap_or_default()
            )
        } else {
            serde_json::from_str(&event.config_json).unwrap_or_default()
        }
    } else {
        serde_json::from_str(&event.config_json).unwrap_or_default()
    };

    let day_max_capacity = if let Some(rule) = override_rule
        && let Some(cap) = rule.override_max_participants {
        cap
    } else {
        event.max_participants
    };

    let mut minute_counts = [0u8; TOTAL_MINUTES];
    let day_start_tz = tz.from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).single().unwrap();
    let day_end_tz = tz.from_local_datetime(&date.and_hms_opt(23, 59, 59).unwrap()).single().unwrap();

    let day_start_utc = day_start_tz.with_timezone(&Utc);
    let day_end_utc = day_end_tz.with_timezone(&Utc);

    let mut earliest_booking_start = None;

    for booking in existing_bookings {
        let b_start = max(booking.start_time, day_start_utc);
        let b_end = min(booking.end_time, day_end_utc);

        if b_start < b_end {
            match earliest_booking_start {
                Some(current_min) if booking.start_time < current_min => {
                    earliest_booking_start = Some(booking.start_time);
                }
                None => {
                    earliest_booking_start = Some(booking.start_time);
                }
                _ => {}
            }

            let start_diff = b_start.timestamp() - day_start_utc.timestamp();
            let end_diff = b_end.timestamp() - day_start_utc.timestamp();

            let s_idx = max(0, min(start_diff / 60, TOTAL_MINUTES as i64)) as usize;
            let e_idx = max(0, min(end_diff / 60, TOTAL_MINUTES as i64)) as usize;

            for count in &mut minute_counts[s_idx..e_idx] {
                *count = count.saturating_add(1);
            }
        }
    }

    let now_utc = Utc::now();
    let cutoff_general = now_utc + Duration::minutes(event.min_notice_general as i64);
    let cutoff_first = now_utc + Duration::minutes(event.min_notice_first as i64);

    let mut valid_slots = Vec::new();
    let duration_min = event.duration_min as usize;
    let interval_min = event.interval_min as usize;

    if duration_min == 0 || interval_min == 0 {
        return Vec::new();
    }

    let daily_windows = match date.weekday() {
        chrono::Weekday::Mon => &config.monday,
        chrono::Weekday::Tue => &config.tuesday,
        chrono::Weekday::Wed => &config.wednesday,
        chrono::Weekday::Thu => &config.thursday,
        chrono::Weekday::Fri => &config.friday,
        chrono::Weekday::Sat => &config.saturday,
        chrono::Weekday::Sun => &config.sunday,
    };

    if let Some(windows) = daily_windows {
        for window in windows {
            // Determine capacity for this window.
            // Hierarchy: Window Specific > Override Specific (Day) > Event Global
            let window_capacity = window.max_participants.unwrap_or(day_max_capacity);

            if let (Ok(start), Ok(end)) = (
                NaiveTime::parse_from_str(&window.start, "%H:%M"),
                NaiveTime::parse_from_str(&window.end, "%H:%M")
            ) {
                let win_start_idx = (start.hour() * 60 + start.minute()) as usize;
                let mut win_end_idx = (end.hour() * 60 + end.minute()) as usize;
                if win_end_idx == 1439 { win_end_idx = 1440; }

                let mut cursor = win_start_idx;
                while cursor + duration_min <= win_end_idx {
                    let hour = (cursor / 60) as u32;
                    let minute = (cursor % 60) as u32;

                    if let Some(nt) = NaiveTime::from_hms_opt(hour, minute, 0)
                        && let Some(slot_tz) = tz.from_local_datetime(&date.and_time(nt)).single() {
                            let slot_utc = slot_tz.with_timezone(&Utc);
                            let slot_end_utc = slot_utc + Duration::minutes(duration_min as i64);

                            let required_cutoff = if let Some(first_start) = earliest_booking_start {
                                if slot_utc > first_start {
                                    cutoff_general
                                } else {
                                    cutoff_first
                                }
                            } else {
                                cutoff_first
                            };

                            // Check capacity for this specific slot duration
                            let mut is_capacity_ok = true;
                            for i in cursor..(cursor + duration_min) {
                                if i < TOTAL_MINUTES && minute_counts[i] as i32 >= window_capacity {
                                    is_capacity_ok = false;
                                    break;
                                }
                            }

                            if slot_utc > required_cutoff
                                && slot_utc >= event.active_start
                                && slot_end_utc <= event.active_end
                                && is_capacity_ok
                            {
                                valid_slots.push(slot_utc.to_rfc3339());
                            }
                        }
                    cursor += interval_min;
                }
            }
        }
    }

    valid_slots.sort();
    valid_slots.dedup();
    valid_slots
}