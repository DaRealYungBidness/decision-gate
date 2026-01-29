// ret-logic/src/plan.rs
// ============================================================================
// Module: Requirement Plan
// Description: Compiled representation of requirement evaluation plans.
// Purpose: Store required columns, operation sequences, and constants for execution.
// Dependencies: serde::{Deserialize, Serialize}, smallvec::SmallVec
// ============================================================================

//! ## Overview
//! `Plan` captures the bytecode-like representation of requirement trees,
//! describing the components to fetch, the operations to run, and the constant pool
//! so evaluation engines can execute deterministically.

// ============================================================================
// SECTION: Imports
// ============================================================================

use serde::Deserialize;
use serde::Serialize;
use smallvec::SmallVec;

// ============================================================================
// SECTION: Column Keys
// ============================================================================

/// Identifies a component column needed for requirement evaluation
///
/// This is resolved at compile time from component type names to dense IDs.
/// The query system uses this to fetch exactly the required component slices.
///
/// # Invariants
/// - Treat the inner value as an opaque identifier; no semantic ordering is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnKey(pub u16);

impl ColumnKey {
    /// Builds a column key from its raw identifier
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Returns the raw column identifier
    #[must_use]
    pub const fn id(&self) -> u16 {
        self.0
    }
}

// ============================================================================
// SECTION: Plan Structure
// ============================================================================

/// Compiled requirement plan optimized for runtime evaluation
///
/// This is the output of the compilation process that transforms human-readable
/// requirements into efficient evaluation sequences. The plan contains:
/// - Required component columns (drives ECS queries)
/// - Optimized operation sequence (enables direct evaluation)
/// - Constant pool for thresholds and parameters
///
/// # Invariants
/// - When constructed via [`Plan::add_column`] or [`PlanBuilder`], `required_columns` contains no
///   duplicates.
/// - Operations are executed in order; structural correctness (balanced groups, valid operands) is
///   the caller's responsibility.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Component columns required for evaluation
    pub(crate) required_columns: SmallVec<[ColumnKey; 8]>,

    /// Sequence of operations to execute
    pub(crate) operations: Vec<Operation>,

    /// Constant pool for numeric values, strings, etc.
    pub(crate) constants: Vec<Constant>,
}

// ============================================================================
// SECTION: Plan Errors
// ============================================================================

/// Errors that can occur while building a [`Plan`]
///
/// # Invariants
/// - None. Variants are self-contained error categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// The constant pool exceeded the maximum representable index.
    ConstantPoolOverflow {
        /// Maximum number of constants allowed.
        max_constants: usize,
        /// Attempted total after insertion.
        attempted: usize,
    },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConstantPoolOverflow {
                max_constants,
                attempted,
            } => write!(
                f,
                "constant pool overflow: attempted {attempted} constants (max {max_constants})"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

// ============================================================================
// SECTION: Plan APIs
// ============================================================================

impl Plan {
    /// Maximum number of constants supported in a plan.
    const MAX_CONSTANTS: usize = u16::MAX as usize + 1;

    /// Creates a new empty plan
    #[must_use]
    pub fn new() -> Self {
        Self {
            required_columns: SmallVec::new(),
            operations: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Returns the component columns this plan requires
    ///
    /// The ECS query system uses this to build efficient queries that fetch
    /// exactly the components needed for evaluation.
    #[must_use]
    pub fn required_columns(&self) -> &[ColumnKey] {
        &self.required_columns
    }

    /// Returns the operation sequence for this plan
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Returns a constant value by index
    #[must_use]
    pub fn constant(&self, index: ConstantIndex) -> Option<&Constant> {
        self.constants.get(usize::from(index.0))
    }

    /// Adds a required column to this plan
    pub fn add_column(&mut self, column: ColumnKey) {
        if !self.required_columns.contains(&column) {
            self.required_columns.push(column);
        }
    }

    /// Adds an operation to this plan
    pub fn add_operation(&mut self, op: Operation) {
        self.operations.push(op);
    }

    /// Adds a constant and returns its index
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::ConstantPoolOverflow`] when the pool exceeds `u16::MAX`.
    pub fn add_constant(&mut self, constant: Constant) -> Result<ConstantIndex, PlanError> {
        let index = self.constants.len();
        if index >= Self::MAX_CONSTANTS {
            return Err(PlanError::ConstantPoolOverflow {
                max_constants: Self::MAX_CONSTANTS,
                attempted: index + 1,
            });
        }

        self.constants.push(constant);
        let index_u16 = u16::try_from(index).map_err(|_| PlanError::ConstantPoolOverflow {
            max_constants: Self::MAX_CONSTANTS,
            attempted: index + 1,
        })?;
        Ok(ConstantIndex(index_u16))
    }
}

// ============================================================================
// SECTION: Plan Defaults
// ============================================================================

impl Default for Plan {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SECTION: Constant Pool Indexes
// ============================================================================

/// Index into the constant pool
///
/// # Invariants
/// - Intended to reference a valid entry in a [`Plan`] constant pool.
/// - No bounds are enforced by the type itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantIndex(pub u16);

// ============================================================================
// SECTION: Operations
// ============================================================================

/// Runtime operation in a compiled plan
///
/// Operations are designed to be efficiently executed in sequence with
/// minimal branching and maximum cache locality.
///
/// # Invariants
/// - Operand interpretation is opcode-specific and must be enforced by the domain.
#[derive(Debug, Clone, Copy)]
pub struct Operation {
    /// Operation type and behavior
    pub opcode: OpCode,

    /// First operand (column index, constant index, etc.)
    pub operand_a: u16,

    /// Second operand
    pub operand_b: u16,

    /// Third operand (for three-operand instructions)
    pub operand_c: u16,
}

impl Operation {
    /// Creates a new operation
    #[must_use]
    pub const fn new(opcode: OpCode, a: u16, b: u16, c: u16) -> Self {
        Self {
            opcode,
            operand_a: a,
            operand_b: b,
            operand_c: c,
        }
    }
}

// ============================================================================
// SECTION: Operation Codes
// ============================================================================

/// Operation codes for compiled requirement plans
///
/// These represent the primitive operations that can be performed during
/// requirement evaluation. Domains register handlers for specific opcodes.
///
/// # Invariants
/// - Stable `repr(u8)` values are used for dispatch table indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    // Logical operations
    /// Begin an AND group
    AndStart = 0,
    /// Close an AND group
    AndEnd = 1,
    /// Begin an OR group
    OrStart = 2,
    /// Close an OR group
    OrEnd = 3,
    /// Logical NOT
    Not = 4,

    // Comparison operations
    /// Floating-point greater-than-or-equal comparison
    FloatGte = 10,
    /// Floating-point less-than-or-equal comparison
    FloatLte = 11,
    /// Floating-point equality comparison
    FloatEq = 12,
    /// Signed integer greater-than-or-equal comparison
    IntGte = 13,
    /// Signed integer less-than-or-equal comparison
    IntLte = 14,
    /// Signed integer equality comparison
    IntEq = 15,

    // Bitwise operations
    /// All required flags must be present
    HasAllFlags = 20,
    /// At least one of the required flags must be present
    HasAnyFlags = 21,
    /// None of the forbidden flags may be present
    HasNoneFlags = 22,

    // Spatial operations
    /// Within numeric range check
    InRange = 30,
    /// Within spatial region check
    InRegion = 31,

    // Domain-specific opcodes start at 100
    /// Marker for domain-specific opcode offsets
    DomainStart = 100,
}

// ============================================================================
// SECTION: Operation Code Helpers
// ============================================================================

impl OpCode {
    /// Returns true if this is a logical grouping operation
    #[must_use]
    pub const fn is_logical_group(&self) -> bool {
        matches!(self, Self::AndStart | Self::AndEnd | Self::OrStart | Self::OrEnd)
    }

    /// Returns true if this is a comparison operation
    #[must_use]
    pub const fn is_comparison(&self) -> bool {
        matches!(
            self,
            Self::FloatGte
                | Self::FloatLte
                | Self::FloatEq
                | Self::IntGte
                | Self::IntLte
                | Self::IntEq
        )
    }
}

// ============================================================================
// SECTION: Constant Pool
// ============================================================================

/// Constants that can be stored in a plan's constant pool
///
/// # Invariants
/// - `String` values are valid UTF-8 by construction.
/// - `Custom` payloads are opaque and domain-defined.
#[derive(Debug, Clone)]
pub enum Constant {
    /// Floating-point value constant
    Float(f32),
    /// Signed integer constant
    Int(i32),
    /// Unsigned integer constant
    UInt(u32),
    /// UTF-8 string constant
    String(String),
    /// Bit-mask constant
    Flags(u64),

    /// Custom domain-specific constant
    Custom(Vec<u8>),
}

// ============================================================================
// SECTION: Constant Accessors
// ============================================================================

impl Constant {
    /// Attempts to interpret this constant as a float
    #[must_use]
    pub const fn as_float(&self) -> Option<f32> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Attempts to interpret this constant as an integer
    #[must_use]
    pub const fn as_int(&self) -> Option<i32> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Attempts to interpret this constant as an unsigned integer
    #[must_use]
    pub const fn as_uint(&self) -> Option<u32> {
        match self {
            Self::UInt(u) => Some(*u),
            _ => None,
        }
    }

    /// Attempts to interpret this constant as flags
    #[must_use]
    pub fn as_flags(&self) -> Option<u64> {
        match self {
            Self::Flags(f) => Some(*f),
            Self::UInt(u) => Some(u64::from(*u)),
            _ => None,
        }
    }

    /// Attempts to interpret this constant as a string
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
}

// ============================================================================
// SECTION: Plan Builder
// ============================================================================

/// Builder for constructing plans programmatically
///
/// # Invariants
/// - Operations and constants are appended in-order to the underlying [`Plan`].
pub struct PlanBuilder {
    /// Accumulated plan being built.
    plan: Plan,
}

// ============================================================================
// SECTION: Plan Builder Methods
// ============================================================================

impl PlanBuilder {
    /// Creates a new plan builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            plan: Plan::new(),
        }
    }

    /// Adds a column requirement
    #[must_use]
    pub fn require_column(mut self, column: ColumnKey) -> Self {
        self.plan.add_column(column);
        self
    }

    /// Adds an operation
    #[must_use]
    pub fn add_op(mut self, opcode: OpCode, a: u16, b: u16, c: u16) -> Self {
        self.plan.add_operation(Operation::new(opcode, a, b, c));
        self
    }

    /// Adds a float constant and returns its index
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::ConstantPoolOverflow`] when the pool exceeds `u16::MAX`.
    pub fn add_float_constant(&mut self, value: f32) -> Result<ConstantIndex, PlanError> {
        self.plan.add_constant(Constant::Float(value))
    }

    /// Adds an integer constant and returns its index
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::ConstantPoolOverflow`] when the pool exceeds `u16::MAX`.
    pub fn add_int_constant(&mut self, value: i32) -> Result<ConstantIndex, PlanError> {
        self.plan.add_constant(Constant::Int(value))
    }

    /// Adds a flags constant and returns its index
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::ConstantPoolOverflow`] when the pool exceeds `u16::MAX`.
    pub fn add_flags_constant(&mut self, flags: u64) -> Result<ConstantIndex, PlanError> {
        self.plan.add_constant(Constant::Flags(flags))
    }

    /// Adds a string constant and returns its index
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::ConstantPoolOverflow`] when the pool exceeds `u16::MAX`.
    pub fn add_string_constant(&mut self, value: String) -> Result<ConstantIndex, PlanError> {
        self.plan.add_constant(Constant::String(value))
    }

    /// Starts an AND group
    #[must_use]
    pub fn and_start(self) -> Self {
        self.add_op(OpCode::AndStart, 0, 0, 0)
    }

    /// Ends an AND group
    #[must_use]
    pub fn and_end(self) -> Self {
        self.add_op(OpCode::AndEnd, 0, 0, 0)
    }

    /// Starts an OR group
    #[must_use]
    pub fn or_start(self) -> Self {
        self.add_op(OpCode::OrStart, 0, 0, 0)
    }

    /// Ends an OR group  
    #[must_use]
    pub fn or_end(self) -> Self {
        self.add_op(OpCode::OrEnd, 0, 0, 0)
    }

    /// Builds the final plan
    #[must_use]
    pub fn build(self) -> Plan {
        self.plan
    }

    /// Adds an operation (mutable borrow)
    pub fn add_op_mut(&mut self, opcode: OpCode, a: u16, b: u16, c: u16) -> &mut Self {
        self.plan.add_operation(Operation::new(opcode, a, b, c));
        self
    }

    /// Adds an integer constant and returns its index (mutable borrow)
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::ConstantPoolOverflow`] when the pool exceeds `u16::MAX`.
    pub fn add_int_constant_mut(&mut self, value: i32) -> Result<ConstantIndex, PlanError> {
        self.plan.add_constant(Constant::Int(value))
    }
}

// ============================================================================
// SECTION: Plan Builder Defaults
// ============================================================================

impl Default for PlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}
