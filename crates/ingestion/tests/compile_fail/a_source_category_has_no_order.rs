//! Section 8.4's six collection targets are a list, not a ranking.
//!
//! The section prints them numbered and then says in the same paragraph that a
//! conflict is not settled by the higher or lower number. So the enum derives
//! no ordering and two values cannot be compared.

use academic_ingestion::SourceCategory;

fn main() {
    let statute = SourceCategory::UniversityRegulations;
    let department = SourceCategory::DepartmentPage;
    let _ranked = statute < department;
}
