pub fn is_prime(n: f64) -> bool {
    if n.is_sign_negative() {
        return false;
    }
    if n.fract() != 0.0 {
        return false;
    }

    // AKS primality test
    let n = n.round() as u32;

    if n <= 1 {
        return false;
    } else if n == 2 || n == 3 {
        return true;
    } else if n % 2 == 0 || n % 3 == 0 {
        return false;
    }

    let r = (n as f32).sqrt() as u32;
    for i in (5..=r).step_by(6) {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
    }

    true
}
