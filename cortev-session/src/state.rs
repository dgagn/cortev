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
