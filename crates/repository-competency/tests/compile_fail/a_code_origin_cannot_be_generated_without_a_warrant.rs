//! `CodeOrigin::Generated` holds the warrant rather than sitting beside it.
//!
//! So generated code with no warrant is not a value somebody has to remember to
//! check — it is a variant that cannot be named without one.
//!
//! The binding is annotated on purpose. `CodeOrigin::Generated` written alone is
//! the tuple variant's **constructor function**, so `let _bare =
//! CodeOrigin::Generated;` compiles and proves nothing; the first draft of this
//! case did exactly that and trybuild reported it as a case that succeeded. What
//! does not exist is a `CodeOrigin` **value** at that variant with the payload
//! left out, and that is what each line below asks for.

use academic_repository_competency::CodeOrigin;

fn main() {
    let _bare: CodeOrigin = CodeOrigin::Generated;
    let _record = CodeOrigin::Generated {};
    let unit = CodeOrigin::HandWritten;
    let _reads_as_bare = matches!(unit, CodeOrigin::Generated);
}
