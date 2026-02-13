pub mod csv;
pub mod json;
pub mod ldif;
pub mod xlsx;

use std::path::Path;

use crate::entry::LdapEntry;
use crate::error::CoreError;
use crate::export::ExportFormat;

/// Import entries from a file, auto-detecting format from extension.
pub fn import_entries(path: &Path) -> Result<Vec<LdapEntry>, CoreError> {
    let format = ExportFormat::from_path(path)
        .ok_or_else(|| CoreError::ImportError("Unknown file extension".to_string()))?;

    match format {
        ExportFormat::Ldif => ldif::import(path),
        ExportFormat::Json => json::import(path),
        ExportFormat::Csv => csv::import(path),
        ExportFormat::Xlsx => xlsx::import(path),
    }
}
