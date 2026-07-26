use crate::event::NotifyEvent;
use serde::Serialize;
use std::time::Instant;

const MAX_VISIBLE: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: String,
    pub title: String,
    pub message: String,
    pub state: String,
    pub progress: f32,
    pub opacity: f32,
    pub route: String,
    pub primary_label: String,
    pub primary_route: String,
    pub secondary_label: String,
    pub secondary_route: String,
    pub sticky: bool,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
    pub dismissing: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToastView {
    pub id: String,
    pub title: String,
    pub message: String,
    pub state: String,
    pub progress: f32,
    pub opacity: f32,
    pub route: String,
    pub primary_label: String,
    pub primary_route: String,
    pub secondary_label: String,
    pub secondary_route: String,
    pub sticky: bool,
    pub dismissing: bool,
}

#[derive(Debug, Default)]
pub struct ToastStore {
    pub toasts: Vec<Toast>,
}

impl ToastStore {
    pub fn upsert(&mut self, event: NotifyEvent) {
        let toast = Toast::from_event(event);
        if let Some(index) = self.toasts.iter().position(|item| item.id == toast.id) {
            self.toasts[index] = toast;
            return;
        }
        self.toasts.insert(0, toast);
    }

    pub fn dismiss(&mut self, id: &str) {
        self.toasts.retain(|toast| toast.id != id);
    }

    pub fn visible(&self) -> Vec<Toast> {
        let now = Instant::now();
        self.toasts
            .iter()
            .filter(|toast| {
                toast.sticky || toast.expires_at.map_or(true, |expires_at| expires_at > now)
            })
            .take(MAX_VISIBLE)
            .cloned()
            .map(|mut toast| {
                toast.opacity = toast.opacity_at(now);
                toast
            })
            .collect()
    }

    pub fn visible_views(&self) -> Vec<ToastView> {
        self.visible().into_iter().map(ToastView::from).collect()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.toasts.len()
    }
}

impl Toast {
    fn from_event(event: NotifyEvent) -> Self {
        let sticky = event.status == "loading";
        let primary_action = event.actions.first();
        let secondary_action = event.actions.get(1);
        let primary_route = primary_action
            .map(|action| action.route.clone())
            .or_else(|| non_empty(event.primary_route.clone()))
            .or_else(|| non_empty(event.route.clone()))
            .unwrap_or_default();
        Self {
            id: event.toast_id,
            title: event.title,
            message: event.message,
            state: event.status,
            progress: event.progress.unwrap_or(0.0),
            opacity: 1.0,
            route: event.route.clone(),
            primary_label: primary_action
                .map(|action| action.label.clone())
                .or_else(|| non_empty(event.primary_label.clone()))
                .or_else(|| (!primary_route.is_empty()).then(|| "Open".into()))
                .unwrap_or_default(),
            primary_route,
            secondary_label: secondary_action
                .map(|action| action.label.clone())
                .or_else(|| non_empty(event.secondary_label.clone()))
                .unwrap_or_default(),
            secondary_route: secondary_action
                .map(|action| action.route.clone())
                .or_else(|| non_empty(event.secondary_route.clone()))
                .unwrap_or_default(),
            sticky,
            created_at: Instant::now(),
            expires_at: None,
            dismissing: false,
        }
    }

    fn opacity_at(&self, now: Instant) -> f32 {
        const FADE_IN_MS: u128 = 220;
        const FADE_OUT_MS: u128 = 800;
        let age = now.duration_since(self.created_at).as_millis();
        let fade_in = if age >= FADE_IN_MS {
            1.0
        } else {
            age as f32 / FADE_IN_MS as f32
        };
        let fade_out = match self.expires_at {
            Some(expires_at) if expires_at <= now => 0.0,
            Some(expires_at) => {
                let remaining = expires_at.duration_since(now).as_millis();
                if remaining >= FADE_OUT_MS {
                    1.0
                } else {
                    remaining as f32 / FADE_OUT_MS as f32
                }
            }
            None => 1.0,
        };
        fade_in.min(fade_out)
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

impl From<Toast> for ToastView {
    fn from(toast: Toast) -> Self {
        Self {
            id: toast.id,
            title: toast.title,
            message: toast.message,
            state: toast.state,
            progress: toast.progress,
            opacity: toast.opacity,
            route: toast.route,
            primary_label: toast.primary_label,
            primary_route: toast.primary_route,
            secondary_label: toast.secondary_label,
            secondary_route: toast.secondary_route,
            sticky: toast.sticky,
            dismissing: toast.dismissing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::parse_event;

    #[test]
    fn duplicate_update_keeps_one_toast() {
        let mut store = ToastStore::default();
        store.upsert(parse_event(r#"{"event":"test","toastId":"a","title":"A"}"#).unwrap());
        store.upsert(parse_event(r#"{"event":"test","toastId":"a","title":"B"}"#).unwrap());
        assert_eq!(store.len(), 1);
        assert_eq!(store.visible()[0].title, "B");
    }

    #[test]
    fn max_five_visible() {
        let mut store = ToastStore::default();
        for index in 0..6 {
            store.upsert(
                parse_event(&format!(r#"{{"event":"test","toastId":"{}"}}"#, index)).unwrap(),
            );
        }
        assert_eq!(store.len(), 6);
        assert_eq!(store.visible().len(), 5);
    }

    #[test]
    fn only_loading_is_sticky() {
        let mut store = ToastStore::default();
        store.upsert(parse_event(r#"{"event":"test","toastId":"a","status":"loading"}"#).unwrap());
        store.upsert(
            parse_event(r#"{"event":"plan.finished","toastId":"b","status":"success"}"#).unwrap(),
        );
        let toasts = store.visible();
        assert!(toasts.iter().find(|toast| toast.id == "a").unwrap().sticky);
        assert!(!toasts.iter().find(|toast| toast.id == "b").unwrap().sticky);
        assert!(toasts
            .iter()
            .find(|toast| toast.id == "b")
            .unwrap()
            .expires_at
            .is_none());
    }

    #[test]
    fn loading_then_success_same_toast_id_updates_one() {
        let mut store = ToastStore::default();
        store.upsert(
            parse_event(r#"{"event":"test.started","toastId":"a","status":"loading"}"#).unwrap(),
        );
        store.upsert(
            parse_event(
                r#"{"event":"test.finished","toastId":"a","status":"success","title":"Pass"}"#,
            )
            .unwrap(),
        );
        let toasts = store.visible();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].state, "success");
        assert_eq!(toasts[0].title, "Pass");
        assert!(!toasts[0].sticky);
    }

    #[test]
    fn non_promise_success_waits_for_renderer_to_close() {
        let mut store = ToastStore::default();
        store.upsert(
            parse_event(r#"{"event":"test.finished","toastId":"a","status":"success"}"#).unwrap(),
        );
        assert_eq!(store.visible().len(), 1);
        store.dismiss("a");
        assert!(store.visible().is_empty());
    }

    #[test]
    fn dismiss_marks_toast_for_fade_out() {
        let mut store = ToastStore::default();
        store.upsert(parse_event(r#"{"event":"test","toastId":"a"}"#).unwrap());
        store.dismiss("a");
        assert_eq!(store.len(), 0);
        assert!(store.visible().is_empty());
    }

    #[test]
    fn maps_primary_and_secondary_actions() {
        let mut store = ToastStore::default();
        store.upsert(parse_event(r#"{"event":"plan.item.finished","toastId":"a","actions":[{"label":"Mở run","route":"/runs/1","kind":"primary"},{"label":"Report","route":"/reports/x","kind":"secondary"}]}"#).unwrap());
        let toast = &store.visible()[0];
        assert_eq!(toast.primary_label, "Mở run");
        assert_eq!(toast.primary_route, "/runs/1");
        assert_eq!(toast.secondary_label, "Report");
        assert_eq!(toast.secondary_route, "/reports/x");
    }

    #[test]
    fn event_without_routes_has_no_default_action() {
        let mut store = ToastStore::default();
        store.upsert(parse_event(r#"{"event":"test","toastId":"a"}"#).unwrap());
        let toast = &store.visible()[0];
        assert_eq!(toast.primary_label, "");
        assert_eq!(toast.primary_route, "");
    }

    #[test]
    fn event_route_becomes_primary_action() {
        let mut store = ToastStore::default();
        store.upsert(
            parse_event(
                r#"{"event":"deploy.finished","toastId":"a","route":"https://example.com/deployments/42"}"#,
            )
            .unwrap(),
        );
        let toast = &store.visible()[0];
        assert_eq!(toast.primary_label, "Open");
        assert_eq!(toast.primary_route, "https://example.com/deployments/42");
    }

    #[test]
    fn visible_views_are_serializable() {
        let mut store = ToastStore::default();
        store.upsert(parse_event(r#"{"event":"test","toastId":"a","progress":0.4}"#).unwrap());
        let json = serde_json::to_string(&store.visible_views()).unwrap();
        assert!(json.contains("\"progress\":0.4"));
    }
}
