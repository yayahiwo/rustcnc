use rustcnc_core::gcode::GCodeFile;
use rustcnc_core::grbl::protocol::MAX_LINE_LENGTH;

/// Validation result for a G-code file
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub warnings: Vec<ValidationWarning>,
    pub errors: Vec<ValidationError>,
}

#[derive(Debug)]
pub struct ValidationWarning {
    pub line: usize,
    pub message: String,
}

#[derive(Debug)]
pub struct ValidationError {
    pub line: usize,
    pub message: String,
}

/// Validate a G-code file before streaming.
/// Checks for issues that would cause GRBL errors.
pub fn validate_gcode(file: &GCodeFile) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for line in &file.lines {
        // Check line length (GRBL has a 256 character limit)
        if line.text.len() > MAX_LINE_LENGTH {
            errors.push(ValidationError {
                line: line.file_line,
                message: format!(
                    "Line exceeds maximum length ({} > {})",
                    line.text.len(),
                    MAX_LINE_LENGTH
                ),
            });
        }

        // Check for unsupported commands
        if line.text.contains("G43") {
            warnings.push(ValidationWarning {
                line: line.file_line,
                message: "G43 tool length offset may not be supported".into(),
            });
        }

        // Check for missing feed rate on first G1
        // (simplified check -- real validation would track modal state)
    }

    let valid = errors.is_empty();

    ValidationResult {
        valid,
        warnings,
        errors,
    }
}
