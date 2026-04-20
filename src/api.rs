use crate::error::PidError;
use crate::model::PidDocument;
use crate::package::PidPackage;
use std::path::Path;

pub struct PidParser {
    options: ParseOptions,
}

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub scan_strings: bool,
    pub parse_xml: bool,
    pub parse_jsite_properties: bool,
    pub keep_unknown_streams: bool,
    pub max_preview_strings: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            scan_strings: true,
            parse_xml: true,
            parse_jsite_properties: true,
            keep_unknown_streams: true,
            max_preview_strings: 64,
        }
    }
}

impl PidParser {
    pub fn new() -> Self {
        Self {
            options: ParseOptions::default(),
        }
    }

    pub fn with_options(options: ParseOptions) -> Self {
        Self { options }
    }

    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<PidDocument, PidError> {
        crate::cfb::reader::parse_pid_file(path.as_ref(), &self.options)
    }

    /// Parse the file and additionally retain every CFB stream's raw bytes,
    /// returning a [`PidPackage`] suitable for round-tripping or feeding
    /// into [`crate::writer::PidWriter`].
    pub fn parse_package<P: AsRef<Path>>(&self, path: P) -> Result<PidPackage, PidError> {
        crate::cfb::reader::parse_pid_package(path.as_ref(), &self.options)
    }
}

impl Default for PidParser {
    fn default() -> Self {
        Self::new()
    }
}
