#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// The session is unchanged since creation.
    #[default]
    Unchanged,
    /// The session's data has been modified.
    Changed,
    /// The session has been regenerated.
    Regenerated,
    /// The session has been invalidated and is no longer valid.
    Invalidated,
}

/// Defines a transition mechanism for states.
pub(crate) trait Transition<T> {
    /// Transitions from the current state to a new state.
    fn transition(self, new_state: T) -> T;
}

impl Transition<SessionState> for SessionState {
    fn transition(self, new_state: SessionState) -> SessionState {
        match (self, new_state) {
            (_, Self::Invalidated) => Self::Invalidated,
            (_, Self::Regenerated) => Self::Regenerated,
            (Self::Unchanged, Self::Changed) => Self::Changed,
            (current, _) => current,
        }
    }
}

impl core::fmt::Display for SessionState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let lowercase = match self {
            SessionState::Unchanged => "unchanged",
            SessionState::Changed => "changed",
            SessionState::Regenerated => "regenerated",
            SessionState::Invalidated => "invalidated",
        };
        write!(f, "{}", lowercase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition() {
        let state = SessionState::Unchanged;
        assert_eq!(
            state.transition(SessionState::Changed),
            SessionState::Changed
        );
        assert_eq!(
            state.transition(SessionState::Regenerated),
            SessionState::Regenerated
        );
        let state = SessionState::Changed;

        assert_eq!(
            state.transition(SessionState::Unchanged),
            SessionState::Changed
        );
        assert_eq!(
            state.transition(SessionState::Regenerated),
            SessionState::Regenerated
        );

        let state = SessionState::Regenerated;
        assert_eq!(
            state.transition(SessionState::Unchanged),
            SessionState::Regenerated
        );

        assert_eq!(
            state.transition(SessionState::Changed),
            SessionState::Regenerated
        );

        let state = SessionState::Invalidated;
        assert_eq!(
            state.transition(SessionState::Unchanged),
            SessionState::Invalidated
        );

        assert_eq!(
            state.transition(SessionState::Changed),
            SessionState::Invalidated
        );

        assert_eq!(
            state.transition(SessionState::Regenerated),
            SessionState::Regenerated
        );
    }
}
