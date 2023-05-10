use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct RError {
    details: String,
    kind: RErrorKind,
}

impl RError {
    pub fn new(error_kind:RErrorKind, msg: &str) -> RError {
        RError{kind:error_kind, details: msg.to_string()}
    }
}

impl fmt::Display for RError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for RError {
    fn description(&self) -> &str {
        &self.details
    }
}

#[derive(Debug)]
pub enum RErrorKind{
    RegexError,
    ConvertionError,
    InputOutputError,
    ReadHeaderError,
}

impl RErrorKind{
    pub fn to_string(&self) -> String{
        match self{
            RErrorKind::RegexError => "RegexError".to_string(),
            RErrorKind::ConvertionError => "ConvertionError".to_string(),
            RErrorKind::InputOutputError => "InputOutputError".to_string(),
            RErrorKind::ReadHeaderError => "ReadHeaderError".to_string(),
        }
    }
}