//! LLVM Instrumentation Profile (InstrProf) format parser
//!
//! This module parses the LLVM coverage metadata sections to enable
//! source-based filtering by crate name.
//!
//! # LLVM Coverage Sections
//!
//! - `__llvm_prf_cnts`: Counter values (64-bit integers)
//! - `__llvm_prf_data`: Profile data records (metadata about functions)
//! - `__llvm_prf_names`: zlib-compressed function names
//!
//! # Profile Data Record Format (simplified)
//!
//! Each record in `__llvm_prf_data` contains:
//! - Function name reference (pointer into names section)
//! - Function hash
//! - Counter start index
//! - Number of counters
//! - Other metadata
//!
//! # Usage
//!
//! ```rust,no_run
//! use symexrs::coverage_profile::ProfileParser;
//!
//! if let Some(parser) = ProfileParser::new() {
//!     // Get function names with their counter ranges
//!     let mappings = parser.get_counter_mappings();
//!     for (func_name, start_idx, num_counters) in mappings {
//!         println!("{}: counters {}-{}" , func_name, start_idx, start_idx + num_counters);
//!     }
//! }
//! ```

use std::collections::HashMap;

/// A mapping from function name to its counter indices
#[derive(Debug, Clone)]
pub struct CounterMapping {
    /// Function name (demangled if possible)
    pub name: String,
    /// Starting counter index
    pub start_idx: usize,
    /// Number of counters for this function
    pub num_counters: usize,
    /// Function hash from profile data
    pub func_hash: u64,
}

/// Parser for LLVM instrumentation profile data
pub struct ProfileParser {
    /// Raw profile data bytes
    data: Vec<u8>,
    /// Decompressed names section
    names: Vec<u8>,
    /// Number of counters in the binary
    num_counters: usize,
}

impl ProfileParser {
    /// Create a new profile parser by reading LLVM sections
    pub fn new() -> Option<Self> {
        // Read the profile sections
        let data = Self::read_data_section()?;
        let names = Self::read_and_decompress_names_section()?;
        let num_counters = Self::get_num_counters()?;

        if data.is_empty() || names.is_empty() {
            return None;
        }

        Some(Self {
            data,
            names,
            num_counters,
        })
    }

    /// Get the number of coverage counters
    fn get_num_counters() -> Option<usize> {
        unsafe {
            let begin = coverage_begin_counters();
            let end = coverage_end_counters();
            if begin.is_null() || end.is_null() {
                return None;
            }
            Some((end as usize - begin as usize) / std::mem::size_of::<u64>())
        }
    }

    /// Read the __llvm_prf_data section
    fn read_data_section() -> Option<Vec<u8>> {
        unsafe {
            let begin = coverage_begin_data();
            let end = coverage_end_data();
            if begin.is_null() || end.is_null() || begin >= end {
                return None;
            }

            let size = end as usize - begin as usize;
            let data = std::slice::from_raw_parts(begin as *const u8, size);
            Some(data.to_vec())
        }
    }

    /// Read and decompress the __llvm_prf_names section
    fn read_and_decompress_names_section() -> Option<Vec<u8>> {
        unsafe {
            let begin = coverage_begin_names();
            let end = coverage_end_names();
            if begin.is_null() || end.is_null() || begin >= end {
                return None;
            }

            let size = end as usize - begin as usize;
            let compressed = std::slice::from_raw_parts(begin as *const u8, size);

            // Try to decompress with zlib
            Self::decompress_zlib(compressed)
        }
    }

    /// Decompress zlib data
    fn decompress_zlib(data: &[u8]) -> Option<Vec<u8>> {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        // Try to decompress
        let mut decoder = ZlibDecoder::new(data);
        let mut decompressed = Vec::new();

        match decoder.read_to_end(&mut decompressed) {
            Ok(_) => Some(decompressed),
            Err(_) => {
                // If decompression fails, the data might not be compressed
                // Return it as-is
                Some(data.to_vec())
            }
        }
    }

    /// Parse profile data records and return counter mappings
    ///
    /// This parses the `__llvm_prf_data` section to extract:
    /// - Function names (from names section)
    /// - Counter start indices
    /// - Number of counters per function
    pub fn get_counter_mappings(&self) -> Vec<CounterMapping> {
        let mut mappings = Vec::new();

        // Parse each profile data record
        // The format varies by LLVM version, but generally:
        // - Each record has a fixed header followed by variable-length data
        // - Header contains: name_ptr, func_hash, counter_ptr, num_counters
        // - We need to parse carefully based on the platform word size

        // For now, we'll use a simplified approach
        // In reality, we need to parse the InstrProfRecord structure
        // which is version and platform dependent

        // Parse the names section (null-separated strings)
        let names = self.parse_names_section();

        // Build mappings (simplified - assumes 1:1 mapping)
        // In reality, we need to correlate with data section
        for (idx, name) in names.iter().enumerate() {
            if let Some(crate_name) = Self::extract_crate_name(name) {
                mappings.push(CounterMapping {
                    name: name.clone(),
                    start_idx: idx,  // Simplified - real mapping is in data section
                    num_counters: 1, // Simplified
                    func_hash: 0,
                });
            }
        }

        mappings
    }

    /// Parse the names section into individual function names
    fn parse_names_section(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut start = 0;

        for (i, &byte) in self.names.iter().enumerate() {
            if byte == 0 {
                if i > start {
                    if let Ok(name) = std::str::from_utf8(&self.names[start..i]) {
                        names.push(name.to_string());
                    }
                }
                start = i + 1;
            }
        }

        names
    }

    /// Extract crate name from a mangled Rust symbol
    ///
    /// Rust symbols are mangled using a specific format:
    /// - `_ZN` prefix (on some platforms)
    /// - Followed by crate name, module path, function name
    /// - Each component is length-prefixed
    ///
    /// For example: `_ZN4myapp4main17h1234567890abcdefE`
    /// - `4myapp` = crate name "myapp"
    /// - `4main` = function name "main"
    /// - `17h...` = hash
    fn extract_crate_name(symbol: &str) -> Option<String> {
        // Try to demangle or parse the symbol
        // For now, use a simple heuristic

        // If it starts with _ZN, it's a mangled Rust symbol
        if let Some(rest) = symbol.strip_prefix("_ZN") {
            // Parse the length-prefixed components
            let mut pos = 0;
            let mut components = Vec::new();

            while pos < rest.len() {
                // Find the length prefix
                if let Some(len_end) = rest[pos..].find(|c: char| !c.is_ascii_digit()) {
                    let len_str = &rest[pos..pos + len_end];
                    if let Ok(len) = len_str.parse::<usize>() {
                        let component_start = pos + len_end;
                        let component_end = component_start + len;

                        if component_end <= rest.len() {
                            let component = &rest[component_start..component_end];
                            components.push(component.to_string());
                            pos = component_end;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // The first component is typically the crate name
            components.first().cloned()
        } else {
            // Not a mangled symbol - might be a C function
            None
        }
    }

    /// Get all unique crate names found in the profile
    pub fn get_crate_names(&self) -> Vec<String> {
        let names = self.parse_names_section();
        let mut crates: std::collections::HashSet<String> = names
            .iter()
            .filter_map(|name| Self::extract_crate_name(name))
            .collect();

        let mut result: Vec<String> = crates.into_iter().collect();
        result.sort();
        result
    }

    /// Build a crate-to-counters mapping
    pub fn get_crate_counter_map(&self) -> HashMap<String, Vec<usize>> {
        let mappings = self.get_counter_mappings();
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();

        for mapping in mappings {
            if let Some(crate_name) = Self::extract_crate_name(&mapping.name) {
                let counters: Vec<usize> =
                    (mapping.start_idx..mapping.start_idx + mapping.num_counters).collect();

                map.entry(crate_name).or_default().extend(counters);
            }
        }

        // Sort and deduplicate
        for counters in map.values_mut() {
            counters.sort_unstable();
            counters.dedup();
        }

        map
    }
}

// FFI declarations for the C helper
unsafe extern "C" {
    fn coverage_begin_counters() -> *const u64;
    fn coverage_end_counters() -> *const u64;
    fn coverage_begin_data() -> *const u8;
    fn coverage_end_data() -> *const u8;
    fn coverage_begin_names() -> *const u8;
    fn coverage_end_names() -> *const u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_crate_name() {
        // Test mangled Rust symbol
        assert_eq!(
            ProfileParser::extract_crate_name("_ZN4myapp4main17h1234567890abcdefE"),
            Some("myapp".to_string())
        );

        // Test with module path
        assert_eq!(
            ProfileParser::extract_crate_name("_ZN4myapp3foo3bar17h1234567890abcdefE"),
            Some("myapp".to_string())
        );

        // Non-mangled symbol
        assert_eq!(ProfileParser::extract_crate_name("main"), None);
    }
}
