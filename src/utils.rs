
pub fn decay_calculator(initial: u32, epochs: u32) -> f64 {
    let b: f64 = 1.0f64 / initial as f64;
    let ln_b = b.log10();

    ln_b / epochs as f64
}
