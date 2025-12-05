pub fn get_default_template(name: &str) -> String {
    match name {
        "confirmation" => include_str!("../../templates/defaults/confirmation.mjml").to_string(),
        "reminder" => include_str!("../../templates/defaults/reminder.mjml").to_string(),
        "cancellation" => include_str!("../../templates/defaults/cancellation.mjml").to_string(),
        "reschedule" => include_str!("../../templates/defaults/reschedule.mjml").to_string(),
        "invitation" => include_str!("../../templates/defaults/invitation.mjml").to_string(),
        _ => format!("<mjml><mj-body><mj-text>Default template for {} not found.</mj-text></mj-body></mjml>", name),
    }
}

pub const DEFAULT_CONFIRMATION_SUBJECT: &str = "Booking Confirmed: {{ event_title }}";
pub const DEFAULT_REMINDER_SUBJECT: &str = "Reminder: {{ event_title }} starts soon";
pub const DEFAULT_CANCELLATION_SUBJECT: &str = "Cancelled: {{ event_title }}";
pub const DEFAULT_RESCHEDULE_SUBJECT: &str = "Rescheduled: {{ event_title }}";
pub const DEFAULT_INVITATION_SUBJECT: &str = "Invitation: {{ event_title }}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_templates_exist() {
        let conf = get_default_template("confirmation");
        assert!(conf.contains("Booking Confirmed"), "Confirmation content mismatch");
        assert!(!conf.contains("Default template for"), "Confirmation fell back to error message");

        let rem = get_default_template("reminder");
        assert!(rem.contains("friendly reminder"), "Reminder content mismatch");

        let cancel = get_default_template("cancellation");
        assert!(cancel.contains("Booking Cancelled"), "Cancellation content mismatch");

        let resched = get_default_template("reschedule");
        assert!(resched.contains("Booking Rescheduled"), "Reschedule content mismatch");

        let invite = get_default_template("invitation");
        assert!(invite.contains("You are invited"), "Invitation content mismatch");
        assert!(invite.contains("Book Your Slot"), "Invitation button mismatch");

        let missing = get_default_template("non_existent");
        assert!(missing.contains("Default template for non_existent not found"));
    }
}