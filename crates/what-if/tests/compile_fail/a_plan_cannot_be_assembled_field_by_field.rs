//! A plan is what the engine computed, or it is nothing.
//!
//! `PlanScenario` has private fields and one producer, `simulate`. A plan
//! assembled by hand could carry a deterministic lane and a projected lane that
//! disagree, or an inputs digest that names inputs it was never computed from.

use academic_what_if::PlanScenario;

fn main() {
    let _plan = PlanScenario {
        id: todo!(),
        basis: todo!(),
        choices: todo!(),
        assumptions: todo!(),
        deterministic: todo!(),
        projections: todo!(),
        inputs_digest: todo!(),
    };
}
