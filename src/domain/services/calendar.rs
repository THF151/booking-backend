use crate::domain::models::{booking::Booking, event::Event};
use icalendar::{Calendar, Component, Event as IcalEvent, EventLike};

/// Generates an iCalendar (.ics) string for a specific booking
pub fn generate_ics(event: &Event, booking: &Booking) -> String {
    let mut calendar = Calendar::new();
    
    let ical_event = IcalEvent::new()
        .summary(&event.title_en)
        .description(&event.desc_en)
        .location(&event.location)
        .starts(booking.start_time)
        .ends(booking.end_time)
        .uid(&booking.id)
        .done(); 

    calendar.push(ical_event);
    calendar.to_string()
}