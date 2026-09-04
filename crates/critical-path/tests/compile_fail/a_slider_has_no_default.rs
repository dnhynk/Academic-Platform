//! Section 16.5: the engine recommends and the user chooses. A shipped neutral
//! preference would be the engine answering the ordering question on the user's
//! behalf, so `PreferenceSlider` has no `Default`.

use academic_critical_path::PreferenceSlider;

fn neutral() -> PreferenceSlider {
    PreferenceSlider::default()
}

fn main() {}
