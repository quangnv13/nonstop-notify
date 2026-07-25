use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotifyEvent {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub event: String,
    #[serde(default)]
    pub toast_id: String,
    #[serde(default)]
    pub group_id: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub message: String,
    #[serde(default = "default_status", alias = "state")]
    pub status: String,
    #[serde(default)]
    pub progress: Option<f32>,
    #[serde(default)]
    pub route: String,
    #[serde(default)]
    pub actions: Vec<NotifyAction>,
    #[serde(default)]
    pub primary_label: String,
    #[serde(default)]
    pub primary_route: String,
    #[serde(default)]
    pub secondary_label: String,
    #[serde(default)]
    pub secondary_route: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotifyAction {
    pub label: String,
    pub route: String,
    #[serde(default = "default_action_kind")]
    pub kind: String,
}

fn default_schema_version() -> u32 {
    1
}
fn default_status() -> String {
    "info".into()
}
fn default_action_kind() -> String {
    "secondary".into()
}

pub fn parse_event(input: &str) -> Result<NotifyEvent, serde_json::Error> {
    serde_json::from_str::<NotifyEvent>(input).map(|mut event| {
        if event.toast_id.trim().is_empty() {
            event.toast_id = format!("event:{}:{}", event.event, event.timestamp);
        }
        if event.title.trim().is_empty() {
            event.title = event.event.clone();
        }
        event.progress = event.progress.map(|value| value.clamp(0.0, 1.0));
        event
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_event() {
        let event = parse_event(r#"{"event":"test.started","toastId":"test:a"}"#).unwrap();
        assert_eq!(event.schema_version, 1);
        assert_eq!(event.toast_id, "test:a");
        assert_eq!(event.title, "test.started");
    }

    #[test]
    fn normalizes_missing_toast_id() {
        let event = parse_event(r#"{"event":"x","timestamp":"t"}"#).unwrap();
        assert_eq!(event.toast_id, "event:x:t");
    }

    #[test]
    fn clamps_progress() {
        let event = parse_event(r#"{"event":"x","toastId":"x","progress":2}"#).unwrap();
        assert_eq!(event.progress, Some(1.0));
    }

    #[test]
    fn accepts_state_alias_for_status() {
        let event = parse_event(r#"{"event":"x","toastId":"x","state":"loading"}"#).unwrap();
        assert_eq!(event.status, "loading");
    }

    #[test]
    fn accepts_flat_action_fields() {
        let event = parse_event(r#"{"event":"x","toastId":"x","primaryLabel":"Open","primaryRoute":"/open","secondaryLabel":"Detail","secondaryRoute":"/detail"}"#).unwrap();
        assert_eq!(event.primary_label, "Open");
        assert_eq!(event.primary_route, "/open");
        assert_eq!(event.secondary_label, "Detail");
        assert_eq!(event.secondary_route, "/detail");
    }
}
