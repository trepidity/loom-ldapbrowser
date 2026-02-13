use std::collections::BTreeMap;
use std::path::Path;

use calamine::{open_workbook, Reader, Xlsx};

use crate::entry::LdapEntry;
use crate::error::CoreError;

/// Import entries from an Excel (.xlsx) file.
///
/// Expects the first sheet with a header row where first column is "dn".
pub fn import(path: &Path) -> Result<Vec<LdapEntry>, CoreError> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|e| CoreError::ImportError(format!("Failed to open Excel file: {}", e)))?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| CoreError::ImportError("Excel file has no sheets".to_string()))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| CoreError::ImportError(format!("Failed to read sheet: {}", e)))?;

    let mut rows = range.rows();

    // First row is header
    let header_row = rows
        .next()
        .ok_or_else(|| CoreError::ImportError("Excel file is empty".to_string()))?;

    let headers: Vec<String> = header_row.iter().map(|cell| cell.to_string()).collect();

    let dn_idx = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case("dn"))
        .ok_or_else(|| CoreError::ImportError("Excel missing 'dn' column".to_string()))?;

    let mut entries = Vec::new();

    for row in rows {
        let cells: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();

        let dn = cells.get(dn_idx).cloned().unwrap_or_default();
        if dn.is_empty() {
            continue;
        }

        let mut attributes = BTreeMap::new();

        for (idx, header) in headers.iter().enumerate() {
            if idx == dn_idx {
                continue;
            }
            if let Some(value) = cells.get(idx) {
                let value = value.trim();
                if !value.is_empty() {
                    let values: Vec<String> = value.split("; ").map(|s| s.to_string()).collect();
                    attributes.insert(header.clone(), values);
                }
            }
        }

        entries.push(LdapEntry::new(dn, attributes));
    }

    Ok(entries)
}
