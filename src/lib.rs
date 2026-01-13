pub mod value;

#[derive(Clone, Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "{self:?}")}
}
impl std::error::Error for Error {}
impl serde::ser::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {Error(msg.to_string())}
}
impl serde::de::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {Error(msg.to_string())}
}

//TODO: Implement Serialize and Deserialize for the Actual Object Notation
