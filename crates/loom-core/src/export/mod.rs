pub mod csv;
pub mod json;
pub mod ldif;
pub mod xlsx;

use std::path::Path;

use crate::entry::LdapEntry;
use crate::error::CoreError;

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Ldif,
    Json,
    Csv,
    Xlsx,
}

impl ExportFormat {
    /// Infer format from file extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "ldif" | "ldf" => Some(Self::Ldif),
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            "xlsx" | "xls" => Some(Self::Xlsx),
            _ => None,
        }
    }
}

/// Export entries to a file, auto-detecting format from extension.
pub fn export_entries(entries: &[LdapEntry], path: &Path) -> Result<usize, CoreError> {
    let format = ExportFormat::from_path(path)
        .ok_or_else(|| CoreError::ExportError("Unknown file extension".to_string()))?;

    match format {
        ExportFormat::Ldif => ldif::export(entries, path),
        ExportFormat::Json => json::export(entries, path),
        ExportFormat::Csv => csv::export(entries, path),
        ExportFormat::Xlsx => xlsx::export(entries, path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_path() {
        assert_eq!(
            ExportFormat::from_path(Path::new("test.ldif")),
            Some(ExportFormat::Ldif)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("test.ldf")),
            Some(ExportFormat::Ldif)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("test.json")),
            Some(ExportFormat::Json)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("test.csv")),
            Some(ExportFormat::Csv)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("test.xlsx")),
            Some(ExportFormat::Xlsx)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("test.xls")),
            Some(ExportFormat::Xlsx)
        );
        assert_eq!(ExportFormat::from_path(Path::new("test.txt")), None);
        assert_eq!(ExportFormat::from_path(Path::new("noext")), None);
    }

    #[test]
    fn test_format_from_path_case_insensitive() {
        assert_eq!(
            ExportFormat::from_path(Path::new("export.LDIF")),
            Some(ExportFormat::Ldif)
        );
        assert_eq!(
            ExportFormat::from_path(Path::new("data.JSON")),
            Some(ExportFormat::Json)
        );
    }
}
