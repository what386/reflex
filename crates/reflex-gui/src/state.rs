use std::time::{Duration, Instant};

const NOTICE_DURATION: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoticeKind {
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DaemonState {
    Checking,
    Connected,
    Disconnected(String),
}

impl DaemonState {
    pub(super) fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Checking => "Checking reflexd",
            Self::Connected => "reflexd connected",
            Self::Disconnected(_) => "reflexd disconnected",
        }
    }
}

#[derive(Debug)]
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) kind: NoticeKind,
    shown_at: Instant,
}

impl Notice {
    pub(super) fn new(kind: NoticeKind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind,
            shown_at: Instant::now(),
        }
    }

    pub(super) fn is_expired_at(&self, now: Instant) -> bool {
        now.duration_since(self.shown_at) >= NOTICE_DURATION
    }
}

#[cfg(test)]
mod tests {
    use super::{DaemonState, Notice, NoticeKind};
    use std::time::Duration;

    #[test]
    fn notices_expire_after_their_display_duration() {
        let notice = Notice::new(NoticeKind::Success, "Started");
        assert!(!notice.is_expired_at(notice.shown_at + Duration::from_secs(4)));
        assert!(notice.is_expired_at(notice.shown_at + Duration::from_secs(5)));
    }

    #[test]
    fn represents_daemon_connection_states() {
        assert!(!DaemonState::Checking.is_connected());
        assert_eq!(DaemonState::Checking.label(), "Checking reflexd");
        assert!(DaemonState::Connected.is_connected());
        assert_eq!(DaemonState::Connected.label(), "reflexd connected");
        let disconnected = DaemonState::Disconnected("unavailable".to_string());
        assert!(!disconnected.is_connected());
        assert_eq!(disconnected.label(), "reflexd disconnected");
    }
}
