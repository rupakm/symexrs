//! Coverage source mapping for filtering by crate paths
//!
//! This module provides functionality to map coverage counters to their source
//! files/crates, enabling source-based filtering of coverage.
//!
//! # Overview
//!
//! LLVM coverage stores metadata in several sections:
//! - `__llvm_prf_cnts`: The actual counter values
//! - `__llvm_prf_data`: Records mapping counters to functions
//! - `__llvm_prf_names`: Compressed function names
//! - `__llvm_covmap`: Coverage mapping for source locations
//!
//! For source-based filtering, we need to map counters to function names, then
//! use those names to identify which crate each counter belongs to.
//!
//! # Current Implementation
//!
//! The current implementation uses a **heuristic approach** based on counter
//! index ranges, which works well in practice because LLVM typically orders
//! counters by compilation unit. See [`SourceMapper::from_heuristic`] for details.
//!
//! A full source-based implementation would require:
//! 1. Parsing the `__llvm_prf_names` section (zlib-compressed)
//! 2. Mapping function names to counter indices via `__llvm_prf_data`
//! 3. Matching function name prefixes to crate names
//!
//! # Crate-Based Filtering
//!
//! For **true source-based filtering by crate name**, use [`SourceMapper::from_crates`]:
//!
//! ```rust,no_run
//! use symexrs::coverage::CoverageTracker;
//! use symexrs::SourceMapper;
//!
//! // Only include counters from specific crates
//! let mapper = SourceMapper::from_crates(&["my_crate", "my_library"]);
//!
//! let tracker = CoverageTracker::new()
//!     .expect("Coverage not available")
//!     .with_filter(mapper.into_filter());
//! ```
//!
//! This parses the LLVM profile data to extract function names, demangles them,
//! and matches crate names to determine which counters to include.

use std::collections::HashSet;

/// Which profile section to read
#[cfg(unix)]
enum SectionKind {
    Names,
    Data,
}

/// Minimal parsed record from `__llvm_prf_data`
struct ProfDataRecord {
    name_ref: u64,
    num_counters: u32,
}

/// Maps coverage counters to their source crates
///
/// This struct provides filtering capabilities based on which crate/source file
/// a coverage counter belongs to. Use it with [`CoverageTracker::with_filter`](crate::coverage::CoverageTracker::with_filter).
///
/// # Example
///
/// ```rust,no_run
/// use symexrs::coverage::CoverageTracker;
/// use symexrs::SourceMapper;
///
/// // Create a source mapper that includes only the first 30% of counters
/// // (heuristic: counters are ordered by compilation unit)
/// let total_counters = 1000;
/// let mapper = SourceMapper::from_heuristic(total_counters, 0.3);
///
/// // Use it to filter coverage
/// let tracker = CoverageTracker::new()
///     .expect("Coverage not available")
///     .with_filter(move |idx| mapper.is_included(idx));
/// ```
#[derive(Debug, Clone)]
pub struct SourceMapper {
    /// Set of counter indices that belong to the whitelisted crates
    whitelisted_counters: HashSet<usize>,
}

impl SourceMapper {
    /// Create a source mapper from a list of specific counter indices
    ///
    /// # Example
    /// ```
    /// use symexrs::SourceMapper;
    ///
    /// let mapper = SourceMapper::from_indices(&[0, 1, 2, 5, 10]);
    /// assert!(mapper.is_included(0));
    /// assert!(mapper.is_included(5));
    /// assert!(!mapper.is_included(3));
    /// ```
    pub fn from_indices(indices: &[usize]) -> Self {
        Self {
            whitelisted_counters: indices.iter().copied().collect(),
        }
    }

    /// Build a whitelist based on counter index patterns (heuristic approach)
    ///
    /// This is useful when source mapping isn't available. It assumes counters
    /// are ordered such that the first N belong to your crate (which is often true
    /// when your crate is the main binary).
    ///
    /// # Arguments
    /// * `num_counters` - Total number of counters
    /// * `fraction` - Fraction of counters to include (0.0 to 1.0)
    ///
    /// # Example
    /// ```
    /// use symexrs::SourceMapper;
    ///
    /// let mapper = SourceMapper::from_heuristic(100, 0.5);
    /// assert!(mapper.is_included(0));
    /// assert!(mapper.is_included(49));
    /// assert!(!mapper.is_included(50));
    /// ```
    pub fn from_heuristic(num_counters: usize, fraction: f64) -> Self {
        let include_count = (num_counters as f64 * fraction.clamp(0.0, 1.0)) as usize;
        let indices: Vec<usize> = (0..include_count).collect();
        Self::from_indices(&indices)
    }

    /// Create a whitelist excluding a prefix of counters
    ///
    /// This is useful when you know the first N counters belong to std/core
    /// and want to exclude them.
    ///
    /// # Arguments
    /// * `start_idx` - First counter index to include
    /// * `end_idx` - Last counter index to include (exclusive)
    pub fn from_range(start_idx: usize, end_idx: usize) -> Self {
        let indices: Vec<usize> = (start_idx..end_idx).collect();
        Self::from_indices(&indices)
    }

    /// Create a whitelist based on crate names
    ///
    /// This performs **true source-based filtering** by:
    /// 1. Parsing the LLVM profile data sections (`__llvm_prf_data`, `__llvm_prf_names`)
    /// 2. Extracting function names and demangling them
    /// 3. Matching crate names to determine which counters to include
    ///
    /// # Arguments
    /// * `crate_names` - List of crate names to include (e.g., `["my_crate", "my_lib"]`)
    ///
    /// # Platform Support
    /// - ✅ Linux: Uses `dlsym` to locate profile sections
    /// - ✅ macOS: Uses C helper with weak symbols
    ///
    /// # Example
    /// ```rust,no_run
    /// use symexrs::SourceMapper;
    ///
    /// // Only include counters from these crates
    /// let mapper = SourceMapper::from_crates(&["symexrs", "my_test_crate"]);
    ///
    /// if mapper.whitelisted_count() > 0 {
    ///     println!("Found {} counters from specified crates", mapper.whitelisted_count());
    /// }
    /// ```
    pub fn from_crates(crate_names: &[&str]) -> Self {
        // Parse the profile sections
        match Self::parse_profile_sections(crate_names) {
            Some(indices) => Self::from_indices(&indices),
            None => {
                // If parsing fails, return an empty mapper
                Self::default()
            }
        }
    }

    /// Parse LLVM profile sections to extract counter mappings
    ///
    /// This reads both `__llvm_prf_names` and `__llvm_prf_data` sections to build
    /// an accurate mapping from function names to counter index ranges.
    #[cfg(unix)]
    fn parse_profile_sections(crate_names: &[&str]) -> Option<Vec<usize>> {
        unsafe {
            // Read sections
            let (names_data, names_base_addr) = Self::read_section(SectionKind::Names)?;
            let (data_data, _) = Self::read_section(SectionKind::Data)?;
            let total_counters = Self::get_total_counters();

            // Parse function names from the names section
            let function_names = Self::parse_names_section(&names_data, names_base_addr);

            if function_names.is_empty() {
                return None;
            }

            // Build MD5 hash → crate name mapping
            // LLVM uses MD5(name) truncated to 64 bits (little-endian) as NameRef
            let whitelist: HashSet<&str> = crate_names.iter().copied().collect();
            let mut matching_hashes: HashSet<u64> = HashSet::new();

            for name in &function_names {
                if let Some(crate_name) = Self::extract_crate_from_symbol(name) {
                    if whitelist.contains(crate_name.as_str()) {
                        let hash = Self::llvm_name_hash(name);
                        matching_hashes.insert(hash);
                    }
                }
            }

            // Parse data records using flexible record detection
            let records =
                Self::parse_data_records_flexible(&data_data, total_counters);

            match records {
                Some(records) => {
                    // Match data records to names using MD5 hash
                    let mut indices = Vec::new();
                    let mut counter_offset = 0usize;

                    for record in &records {
                        if matching_hashes.contains(&record.name_ref) {
                            for i in 0..record.num_counters as usize {
                                indices.push(counter_offset + i);
                            }
                        }
                        counter_offset += record.num_counters as usize;
                    }

                    if indices.is_empty() { None } else { Some(indices) }
                }
                None => {
                    None
                }
            }
        }
    }

    #[cfg(not(unix))]
    fn parse_profile_sections(_crate_names: &[&str]) -> Option<Vec<usize>> {
        None
    }

    /// Read an LLVM profile section from the running binary.
    /// Returns (data, base_address) where base_address is the section's start address in memory.
    #[cfg(unix)]
    unsafe fn read_section(kind: SectionKind) -> Option<(Vec<u8>, usize)> {
        unsafe extern "C" {
            fn coverage_begin_names() -> *const u8;
            fn coverage_end_names() -> *const u8;
            fn coverage_begin_data() -> *const u8;
            fn coverage_end_data() -> *const u8;
        }

        let (begin, end) = unsafe {
            match kind {
                SectionKind::Names => (coverage_begin_names(), coverage_end_names()),
                SectionKind::Data => (coverage_begin_data(), coverage_end_data()),
            }
        };

        if begin.is_null() || end.is_null() || begin >= end {
            return None;
        }

        let size = end as usize - begin as usize;
        let base_addr = begin as usize;
        Some((
            unsafe { std::slice::from_raw_parts(begin, size).to_vec() },
            base_addr,
        ))
    }

    /// Get total number of counters from the counter section
    #[cfg(unix)]
    unsafe fn get_total_counters() -> usize {
        unsafe extern "C" {
            fn coverage_get_num_counters() -> usize;
        }
        unsafe { coverage_get_num_counters() }
    }

    /// Parse the `__llvm_prf_names` section
    ///
    /// The section contains one or more blocks, each with format:
    /// ```text
    /// [compressed_len: u64 LE] [uncompressed_len: u64 LE] [data...]
    /// [padding to 8-byte alignment]
    /// ```
    ///
    /// If `compressed_len > 0`, data is zlib-compressed (`compressed_len` bytes).
    /// If `compressed_len == 0`, data is raw (`uncompressed_len` bytes).
    /// Names within each block are separated by `\x01`.
    fn parse_names_section(data: &[u8], base_addr: usize) -> Vec<String> {
        // Try the standard LLVM format with ULEB128 headers
        let names = Self::parse_names_uleb128(data, base_addr);

        // If we got a reasonable number of names, use them.
        // Heuristic: if we got fewer than 10 names from a section > 1KB,
        // the ULEB128 parsing likely failed on subsequent blocks.
        if names.len() >= 10 || (names.len() > 0 && data.len() < 1024) {
            return names;
        }

        // Fallback: scan for zlib streams and decompress each one.
        let scan_names = Self::parse_names_by_zlib_scan(data);

        // Use whichever method found more names
        if scan_names.len() > names.len() {
            scan_names
        } else {
            names
        }
    }

    /// Parse names section using ULEB128 block headers.
    ///
    /// Tries both LLVM header formats:
    /// - Standard: `[DataSize][UncompressedSize]` (DataSize bytes follow)
    /// - Reversed: `[UncompressedSize][CompressedSize]` (CompressedSize bytes follow)
    ///
    /// LLVM 20 appears to use the reversed format.
    fn parse_names_uleb128(data: &[u8], base_addr: usize) -> Vec<String> {
        // Try all combinations of format and alignment strategy.
        // LLVM versions differ in header field order and alignment behavior.
        let mut best = Vec::new();

        for &reversed in &[false, true] {
            for &align in &[true, false] {
                let names = Self::parse_names_uleb128_impl(data, base_addr, reversed, align);
                if names.len() > best.len() {
                    best = names;
                }
            }
        }

        best
    }

    fn parse_names_uleb128_impl(
        data: &[u8],
        base_addr: usize,
        reversed: bool,
        use_alignment: bool,
    ) -> Vec<String> {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        let mut names = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            let (first, bytes_read) = Self::decode_uleb128(&data[pos..]);
            if bytes_read == 0 {
                break;
            }
            pos += bytes_read;

            if pos >= data.len() {
                break;
            }
            let (second, bytes_read) = Self::decode_uleb128(&data[pos..]);
            if bytes_read == 0 {
                break;
            }
            pos += bytes_read;

            // Interpret based on format:
            // Standard: first=DataSize, second=UncompressedSize
            //   DataSize bytes follow. If UncompressedSize>0, zlib-compressed.
            // Reversed: first=UncompressedSize, second=CompressedSize
            //   CompressedSize bytes follow. If UncompressedSize>0, zlib-compressed.
            let (data_size, is_compressed) = if reversed {
                let uncompressed_size = first as usize;
                let compressed_size = second as usize;
                if uncompressed_size > 0 {
                    (compressed_size, true)
                } else {
                    (compressed_size, false)
                }
            } else {
                let data_size = first as usize;
                let uncompressed_size = second as usize;
                (data_size, uncompressed_size > 0)
            };

            if data_size == 0 || data_size > data.len().saturating_sub(pos) {
                break;
            }

            let block_data = &data[pos..pos + data_size];
            pos += data_size;

            let names_bytes = if is_compressed {
                let mut decoder = ZlibDecoder::new(block_data);
                let mut decompressed = Vec::new();
                match decoder.read_to_end(&mut decompressed) {
                    Ok(_) => decompressed,
                    Err(_) => {
                        // Decompression failed, this format is probably wrong
                        return names;
                    }
                }
            } else {
                block_data.to_vec()
            };

            Self::split_names_into(&names_bytes, &mut names);

            if use_alignment {
                // Align to 8-byte boundary based on absolute address
                let abs_addr = base_addr.wrapping_add(pos);
                let aligned = (abs_addr + 7) & !7;
                pos = aligned.wrapping_sub(base_addr);
            }
        }

        names
    }

    /// Parse names by scanning for zlib streams in the section data.
    /// This is a robust fallback that doesn't depend on header format.
    fn parse_names_by_zlib_scan(data: &[u8]) -> Vec<String> {
        use flate2::read::ZlibDecoder;
        use std::io::{Cursor, Read};

        let mut names = Vec::new();
        let mut pos = 0;

        while pos + 2 <= data.len() {
            // Look for zlib magic bytes: 0x78 followed by 0x01/0x5E/0x9C/0xDA
            if data[pos] == 0x78 && matches!(data[pos + 1], 0x01 | 0x5E | 0x9C | 0xDA) {
                // Try to decompress from this offset
                let cursor = Cursor::new(&data[pos..]);
                let mut decoder = ZlibDecoder::new(cursor);
                let mut decompressed = Vec::new();

                if decoder.read_to_end(&mut decompressed).is_ok() && !decompressed.is_empty() {
                    // Check if decompressed data looks like function names (valid UTF-8)
                    if let Ok(text) = std::str::from_utf8(&decompressed) {
                        if text.chars().all(|c| c.is_ascii_graphic() || c == '\x01') {
                            Self::split_names_into(&decompressed, &mut names);

                            // Advance past the consumed bytes
                            let consumed = decoder.into_inner().position() as usize;
                            pos += consumed;
                            continue;
                        }
                    }
                }
            }
            pos += 1;
        }

        names
    }

    /// Split decompressed name data by \x01 separator and add to names vec
    fn split_names_into(data: &[u8], names: &mut Vec<String>) {
        for name_bytes in data.split(|&b| b == 0x01) {
            if !name_bytes.is_empty() {
                if let Ok(name) = std::str::from_utf8(name_bytes) {
                    names.push(name.to_string());
                }
            }
        }
    }

    /// Decode a ULEB128-encoded value from a byte slice.
    /// Returns (value, bytes_consumed). Returns (0, 0) on error.
    fn decode_uleb128(data: &[u8]) -> (u64, usize) {
        let mut result: u64 = 0;
        let mut shift = 0;
        for (i, &byte) in data.iter().enumerate() {
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                return (result, i + 1);
            }
            shift += 7;
            if shift >= 64 {
                return (0, 0); // overflow
            }
        }
        (0, 0) // ran out of data
    }

    /// Parse `__llvm_prf_data` records, extracting NameRef and NumCounters.
    ///
    /// Auto-detects record size and field offsets by trying common LLVM layouts
    /// and validating that `sum(NumCounters) == total_counters`.
    /// NameRef (MD5 hash) is always at offset 0 in all known layouts.
    fn parse_data_records_flexible(
        data: &[u8],
        total_counters: usize,
    ) -> Option<Vec<ProfDataRecord>> {
        if data.is_empty() || total_counters == 0 {
            return None;
        }

        // Try common record sizes (LLVM 14-20)
        for &record_size in &[48usize, 56, 64, 72, 40] {
            if data.len() % record_size != 0 {
                continue;
            }
            let num_records = data.len() / record_size;

            // Try known NumCounters offsets
            for &nc_offset in &[40usize, 48, 32, 52, 44, 36] {
                if nc_offset + 4 > record_size {
                    continue;
                }

                let mut records = Vec::new();
                let mut total = 0u64;
                let mut valid = true;

                for i in 0..num_records {
                    let base = i * record_size;

                    // NameRef is always at offset 0 (u64 LE)
                    let name_ref = u64::from_le_bytes(
                        data[base..base + 8].try_into().unwrap(),
                    );

                    let num_counters = u32::from_le_bytes(
                        data[base + nc_offset..base + nc_offset + 4]
                            .try_into()
                            .unwrap(),
                    );

                    if num_counters > 100_000 {
                        valid = false;
                        break;
                    }

                    total += num_counters as u64;
                    records.push(ProfDataRecord {
                        name_ref,
                        num_counters,
                    });
                }

                if valid && total == total_counters as u64 {
                    return Some(records);
                }
            }
        }

        None
    }

    /// Compute LLVM's NameRef hash for a function name.
    /// LLVM uses MD5(name) truncated to 64 bits (little-endian).
    fn llvm_name_hash(name: &str) -> u64 {
        let digest = md5::compute(name.as_bytes());
        u64::from_le_bytes(digest[..8].try_into().unwrap())
    }

    /// Extract crate name from a symbol name
    ///
    /// Supports:
    /// - Legacy mangling (`_ZN` prefix)
    /// - v0 mangling (`_R` prefix)
    /// - Unmangled Rust paths (`crate::module::function`)
    fn extract_crate_from_symbol(symbol: &str) -> Option<String> {
        // Try legacy mangling (_ZN prefix)
        if let Some(rest) = symbol.strip_prefix("_ZN") {
            return Self::parse_legacy_mangling(rest);
        }

        // Try v0 mangling (_R prefix)
        if let Some(rest) = symbol.strip_prefix("_R") {
            return Self::parse_v0_mangling(rest);
        }

        // Try unmangled Rust path (crate::module::function)
        if symbol.contains("::") {
            let crate_name = symbol.split("::").next()?;
            if !crate_name.is_empty()
                && crate_name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Some(crate_name.to_string());
            }
        }

        None
    }

    /// Parse legacy Rust mangling format (_ZN...)
    fn parse_legacy_mangling(s: &str) -> Option<String> {
        let mut pos = 0;
        let mut components = Vec::new();

        while pos < s.len() {
            // Find length prefix
            let len_str: String = s[pos..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();

            if len_str.is_empty() {
                break;
            }

            let len = len_str.parse::<usize>().ok()?;
            let len_digits = len_str.len();
            let component_start = pos + len_digits;
            let component_end = component_start + len;

            if component_end > s.len() {
                break;
            }

            let component = &s[component_start..component_end];
            components.push(component.to_string());
            pos = component_end;
        }

        // First component is typically the crate name
        components.into_iter().next()
    }

    /// Parse v0 Rust mangling format (_R...)
    ///
    /// v0 format (RFC 2603): the crate root is encoded as `C <disambiguator> <name>`
    /// where `<disambiguator>` is `s<base62>_` and `<name>` is `<decimal-length><identifier>`.
    ///
    /// We scan for `C` markers followed by a disambiguator and extract the crate name.
    fn parse_v0_mangling(s: &str) -> Option<String> {
        // Find all occurrences of 'C' which marks a crate root in v0 mangling.
        // The pattern is: C <disambiguator> <length> <name>
        // where disambiguator is: s <base62-chars> _
        // and base62-chars are [0-9a-zA-Z]
        let bytes = s.as_bytes();
        for i in 0..bytes.len() {
            if bytes[i] != b'C' {
                continue;
            }

            // After 'C', expect optional disambiguator: 's' <base62> '_'
            let mut pos = i + 1;
            if pos < bytes.len() && bytes[pos] == b's' {
                pos += 1;
                // Skip base62 chars
                while pos < bytes.len()
                    && (bytes[pos].is_ascii_alphanumeric())
                {
                    pos += 1;
                }
                // Expect '_'
                if pos >= bytes.len() || bytes[pos] != b'_' {
                    continue;
                }
                pos += 1; // skip '_'
            }

            // Now expect: <decimal-length> <identifier>
            let len_start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            if pos == len_start {
                continue; // no length found
            }

            let len: usize = s[len_start..pos].parse().ok()?;
            if pos + len > bytes.len() {
                continue;
            }

            let name = &s[pos..pos + len];
            // Verify it looks like a valid crate name
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Some(name.to_string());
            }
        }

        None
    }

    /// Check if a counter index is in the whitelist
    pub fn is_included(&self, idx: usize) -> bool {
        self.whitelisted_counters.contains(&idx)
    }

    /// Get all whitelisted counter indices
    pub fn whitelisted_indices(&self) -> &HashSet<usize> {
        &self.whitelisted_counters
    }

    /// Get the number of whitelisted counters
    pub fn whitelisted_count(&self) -> usize {
        self.whitelisted_counters.len()
    }

    /// Create a boxed filter function compatible with CoverageTracker::with_filter
    ///
    /// This consumes the SourceMapper and returns a boxed filter function that can
    /// be passed to CoverageTracker::with_filter.
    ///
    /// # Example
    /// ```rust,no_run
    /// use symexrs::coverage::CoverageTracker;
    /// use symexrs::SourceMapper;
    ///
    /// let mapper = SourceMapper::from_heuristic(1000, 0.5);
    /// let tracker = CoverageTracker::new()
    ///     .expect("Coverage not available")
    ///     .with_filter(mapper.into_filter());
    /// ```
    pub fn into_filter(self) -> Box<dyn Fn(usize) -> bool + Send + Sync> {
        let whitelist = self.whitelisted_counters.clone();
        Box::new(move |idx| whitelist.contains(&idx))
    }
}

impl Default for SourceMapper {
    fn default() -> Self {
        Self {
            whitelisted_counters: HashSet::new(),
        }
    }
}

/// Builder for creating source mappers with complex filtering logic
///
/// # Example
/// ```
/// use symexrs::coverage_source_map::SourceMapperBuilder;
///
/// let mapper = SourceMapperBuilder::new()
///     .include_range(0, 100)      // Include counters 0-99
///     .exclude_indices(&[50, 51]) // But exclude 50 and 51
///     .build();
///
/// assert!(mapper.is_included(0));
/// assert!(mapper.is_included(99));
/// assert!(!mapper.is_included(50));
/// assert!(!mapper.is_included(100));
/// ```
#[derive(Debug, Default)]
pub struct SourceMapperBuilder {
    included: HashSet<usize>,
}

impl SourceMapperBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            included: HashSet::new(),
        }
    }

    /// Include a specific counter index
    pub fn include(mut self, idx: usize) -> Self {
        self.included.insert(idx);
        self
    }

    /// Include a range of counter indices [start, end)
    pub fn include_range(mut self, start: usize, end: usize) -> Self {
        for idx in start..end {
            self.included.insert(idx);
        }
        self
    }

    /// Exclude specific indices (removes them if present)
    pub fn exclude_indices(mut self, indices: &[usize]) -> Self {
        for idx in indices {
            self.included.remove(idx);
        }
        self
    }

    /// Build the SourceMapper
    pub fn build(self) -> SourceMapper {
        SourceMapper {
            whitelisted_counters: self.included,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_indices() {
        let mapper = SourceMapper::from_indices(&[0, 1, 2, 5, 10]);
        assert!(mapper.is_included(0));
        assert!(mapper.is_included(5));
        assert!(!mapper.is_included(3));
        assert!(!mapper.is_included(100));
        assert_eq!(mapper.whitelisted_count(), 5);
    }

    #[test]
    fn test_from_heuristic() {
        let mapper = SourceMapper::from_heuristic(100, 0.5);
        assert!(mapper.is_included(0));
        assert!(mapper.is_included(49));
        assert!(!mapper.is_included(50));
        assert!(!mapper.is_included(99));
        assert_eq!(mapper.whitelisted_count(), 50);
    }

    #[test]
    fn test_from_range() {
        let mapper = SourceMapper::from_range(10, 20);
        assert!(!mapper.is_included(9));
        assert!(mapper.is_included(10));
        assert!(mapper.is_included(19));
        assert!(!mapper.is_included(20));
        assert_eq!(mapper.whitelisted_count(), 10);
    }

    #[test]
    fn test_builder() {
        let mapper = SourceMapperBuilder::new()
            .include_range(0, 100)
            .exclude_indices(&[50, 51])
            .build();

        assert!(mapper.is_included(0));
        assert!(mapper.is_included(99));
        assert!(!mapper.is_included(50));
        assert!(!mapper.is_included(51));
        assert!(!mapper.is_included(100));
    }

    #[test]
    fn test_into_filter() {
        let mapper = SourceMapper::from_indices(&[1, 3, 5]);
        let filter = mapper.into_filter();

        assert!(!filter(0));
        assert!(filter(1));
        assert!(!filter(2));
        assert!(filter(3));
        assert!(!filter(4));
        assert!(filter(5));
    }

    #[test]
    fn test_extract_crate_from_symbol_legacy() {
        // Test legacy mangling (_ZN prefix)
        // Format: _ZN<len><name><len><name>...E
        assert_eq!(
            SourceMapper::extract_crate_from_symbol("_ZN5myapp4main17h1234567890abcdefE"),
            Some("myapp".to_string())
        );

        assert_eq!(
            SourceMapper::extract_crate_from_symbol("_ZN3std2io5stdio6_print17h1234567890abcdefE"),
            Some("std".to_string())
        );
    }

    #[test]
    fn test_parse_names_section_uncompressed() {
        // Build a valid LLVM names section block with ULEB128 headers:
        // [data_size: ULEB128] [uncompressed_size: ULEB128 = 0 means not compressed]
        // [data with \x01 separators] [padding to 8-byte alignment]
        let names_str = b"_ZN5myapp4main17habcdefE\x01_ZN3std2io5stdio6_print17habcdefE";

        let mut data = Vec::new();
        // data_size = names_str.len() as ULEB128 (fits in one byte since < 128)
        assert!(names_str.len() < 128);
        data.push(names_str.len() as u8); // data_size
        data.push(0); // uncompressed_size = 0 means not compressed
        data.extend_from_slice(names_str);
        // Pad to 8-byte alignment
        while data.len() % 8 != 0 {
            data.push(0);
        }

        // Use base_addr=0 so alignment is trivial (section-relative == absolute)
        let names = SourceMapper::parse_names_section(&data, 0);
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "_ZN5myapp4main17habcdefE");
        assert_eq!(names[1], "_ZN3std2io5stdio6_print17habcdefE");
    }

    #[test]
    fn test_decode_uleb128() {
        // Single byte
        assert_eq!(SourceMapper::decode_uleb128(&[0x00]), (0, 1));
        assert_eq!(SourceMapper::decode_uleb128(&[0x61]), (97, 1));
        assert_eq!(SourceMapper::decode_uleb128(&[0x7F]), (127, 1));
        // Multi-byte: 128 = 0x80 0x01
        assert_eq!(SourceMapper::decode_uleb128(&[0x80, 0x01]), (128, 2));
        // Multi-byte: 624485 = 0xE5 0x8E 0x26
        assert_eq!(
            SourceMapper::decode_uleb128(&[0xE5, 0x8E, 0x26]),
            (624485, 3)
        );
    }

    #[test]
    fn test_extract_crate_from_symbol_v0() {
        // v0 mangling format: _R <path>
        // Crate root: C <disambiguator> <name>
        assert_eq!(
            SourceMapper::extract_crate_from_symbol(
                "_RNvCs1v6ghPAqC9J_3log9max_level"
            ),
            Some("log".to_string())
        );
        assert_eq!(
            SourceMapper::extract_crate_from_symbol(
                "_RNvMNtCsao6uO1f0bfV_2z33astNtB2_3Int8from_str"
            ),
            Some("z3".to_string())
        );
        assert_eq!(
            SourceMapper::extract_crate_from_symbol(
                "_RINvCsaQRQSrH9COt_7symexrs7exploreNCNvCs3mo2BqeNhOe_13mcts_coverage22run_simple_exploration0EBA_"
            ),
            Some("symexrs".to_string())
        );
    }

    #[test]
    fn test_extract_crate_from_unmangled() {
        assert_eq!(
            SourceMapper::extract_crate_from_symbol("myapp::main"),
            Some("myapp".to_string())
        );
        assert_eq!(
            SourceMapper::extract_crate_from_symbol("std::io::stdio::_print"),
            Some("std".to_string())
        );
    }

    #[test]
    fn test_parse_data_records() {
        // Simulate 2 data records with record_size=48 and NumCounters at offset 40
        // Each record: [NameRef:8][FuncHash:8][CounterPtr:8][FuncPtr:8][Values:8][NumCounters:4][ValueSites:4]
        let mut data = vec![0u8; 96]; // 2 records * 48 bytes

        // Record 0: NameRef = 0x1234, NumCounters = 3
        data[0..8].copy_from_slice(&0x1234u64.to_le_bytes());
        data[40..44].copy_from_slice(&3u32.to_le_bytes());
        // Record 1: NameRef = 0x5678, NumCounters = 5
        data[48..56].copy_from_slice(&0x5678u64.to_le_bytes());
        data[88..92].copy_from_slice(&5u32.to_le_bytes());

        let records = SourceMapper::parse_data_records_flexible(&data, 8);
        assert!(records.is_some());
        let records = records.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name_ref, 0x1234);
        assert_eq!(records[0].num_counters, 3);
        assert_eq!(records[1].name_ref, 0x5678);
        assert_eq!(records[1].num_counters, 5);
    }
}
