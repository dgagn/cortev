#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CookieKind {
    Normal,
    Signed,
    Private,
}
