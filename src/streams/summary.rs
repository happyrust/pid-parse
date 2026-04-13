use crate::error::PidError;
use crate::model::PidDocument;
use std::io::Read;

pub fn parse_summary_streams<R: Read + std::io::Seek>(
    _cfb: &mut ::cfb::CompoundFile<R>,
    _doc: &mut PidDocument,
) -> Result<(), PidError> {
    Ok(())
}
