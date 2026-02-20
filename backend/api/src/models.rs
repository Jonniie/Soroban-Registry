// src/models.rs
// Shared data types for the Soroban Security Audit system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────
// Checklist definition types (static / compile-time)
// ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum CheckCategory {
    InputValidation,
    StateManagement,
    AccessControl,
    Reentrancy,
    NumericalSafety,
    AuthenticationAuthorization,
    DataSerialization,
    ErrorHandling,
    StoragePatterns,
    TokenSafety,
    EventLogging,
    Upgradeability,
    CrossContractCalls,
    ResourceLimits,
}

impl std::fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CheckCategory::InputValidation => "Input Validation",
            CheckCategory::StateManagement => "State Management",
            CheckCategory::AccessControl => "Access Control",
            CheckCategory::Reentrancy => "Reentrancy",
            CheckCategory::NumericalSafety => "Numerical Safety",
            CheckCategory::AuthenticationAuthorization => "Authentication & Authorization",
            CheckCategory::DataSerialization => "Data Serialization",
            CheckCategory::ErrorHandling => "Error Handling",
            CheckCategory::StoragePatterns => "Storage Patterns",
            CheckCategory::TokenSafety => "Token Safety",
            CheckCategory::EventLogging => "Event Logging",
            CheckCategory::Upgradeability => "Upgradeability",
            CheckCategory::CrossContractCalls => "Cross-Contract Calls",
            CheckCategory::ResourceLimits => "Resource Limits",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectionMethod {
    Automatic { patterns: Vec<String> },
    Manual,
    SemiAutomatic { patterns: Vec<String> },
}

/// A static checklist item definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: &'static str,
    pub category: CheckCategory,
    pub title: &'static str,
    pub description: &'static str,
    pub severity: Severity,
    pub detection: DetectionMethod,
    pub remediation: &'static str,
    pub references: Vec<&'static str>,
}

// ─────────────────────────────────────────────────────────
// Runtime / database types
// ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotApplicable,
    Pending,
}

impl Default for CheckStatus {
    fn default() -> Self {
        CheckStatus::Pending
    }
}

/// One row in `audit_checks` — per-check status for an audit
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditCheckRow {
    pub id: Uuid,
    pub audit_id: Uuid,
    pub check_id: String,
    pub status: CheckStatus,
    pub notes: Option<String>,
    pub auto_detected: bool,
    pub evidence: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// One row in `security_audits`
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditRecord {
    pub id: Uuid,
    pub contract_id: Uuid,
    pub contract_source: Option<String>, // raw source for auto-detection
    pub auditor: String,
    pub audit_date: DateTime<Utc>,
    pub overall_score: f64,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────
// API request / response types
// ─────────────────────────────────────────────────────────

/// Body for POST /contracts/:id/security-audit
#[derive(Debug, Deserialize)]
pub struct CreateAuditRequest {
    /// Auditor name or Stellar address
    pub auditor: String,
    /// Optional: paste the contract source for auto-detection
    pub source_code: Option<String>,
}

/// Body for PATCH /contracts/:id/security-audit/:audit_id/checks/:check_id
#[derive(Debug, Deserialize)]
pub struct UpdateCheckRequest {
    pub status: CheckStatus,
    pub notes: Option<String>,
}

/// Full audit response sent to client — includes static metadata + live status
#[derive(Debug, Serialize)]
pub struct AuditResponse {
    pub audit: AuditRecord,
    pub checks: Vec<CheckWithStatus>,
    pub category_scores: Vec<CategoryScore>,
    pub auto_detected_count: usize,
}

/// A checklist item merged with its current audit status
#[derive(Debug, Serialize)]
pub struct CheckWithStatus {
    // ── static metadata ──
    pub id: &'static str,
    pub category: String,
    pub title: &'static str,
    pub description: &'static str,
    pub severity: String,
    pub detection_type: &'static str,
    pub auto_patterns: Vec<String>,
    pub remediation: &'static str,
    pub references: Vec<&'static str>,
    // ── live audit status ──
    pub status: CheckStatus,
    pub notes: Option<String>,
    pub auto_detected: bool,
    pub evidence: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryScore {
    pub category: String,
    pub score: f64,
    pub passed: usize,
    pub total: usize,
    pub failed_critical: usize,
    pub failed_high: usize,
}

/// Minimal score info embedded on contract cards
#[derive(Debug, Serialize, FromRow)]
pub struct ContractSecuritySummary {
    pub audit_id: Uuid,
    pub audit_date: DateTime<Utc>,
    pub auditor: String,
    pub overall_score: f64,
    pub score_badge: String,
}

/// Markdown export request
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    /// Include full check descriptions in export
    #[serde(default = "default_true")]
    pub include_descriptions: bool,
    /// Include only failed/pending checks
    #[serde(default)]
    pub failures_only: bool,
}

fn default_true() -> bool {
    true
}