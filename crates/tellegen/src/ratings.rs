//! How a stated branch or winding rating says "no thermal bound".
//!
//! PowerIO 0.11 states no bound as a nonpositive rating, as MATPOWER does.
//! PowerIO 0.12 states it as positive infinity and reads a numeric zero as a
//! zero apparent-power bound; its readers translate each format's own
//! convention on the way in. Every place Tellegen decides whether a rating
//! limits a branch goes through this one function, so a build on either
//! release line reads the network the way that line wrote it.

/// Whether `rate` means the branch has no thermal bound.
pub fn unlimited(rate: f64) -> bool {
    #[cfg(feature = "powerio-012")]
    {
        rate.is_infinite() && rate > 0.0
    }
    #[cfg(not(feature = "powerio-012"))]
    {
        rate <= 0.0
    }
}

/// A rating for a payload that states "no bound" as `0`: the stated rating
/// scaled by `scale`, or zero when the rating means no bound. The browser and
/// HTTP payloads keep this spelling on both release lines.
pub fn rate_mw(rate: f64, scale: f64) -> f64 {
    if unlimited(rate) {
        0.0
    } else {
        rate * scale
    }
}

#[cfg(test)]
mod tests {
    use super::{rate_mw, unlimited};

    #[test]
    fn a_payload_states_no_bound_as_zero() {
        assert_eq!(rate_mw(2.5, 100.0), 250.0);
        #[cfg(feature = "powerio-012")]
        assert_eq!(rate_mw(f64::INFINITY, 100.0), 0.0);
        #[cfg(not(feature = "powerio-012"))]
        assert_eq!(rate_mw(0.0, 100.0), 0.0);
    }

    #[test]
    fn a_stated_rating_limits_and_the_line_convention_does_not() {
        assert!(!unlimited(250.0));
        assert!(!unlimited(1e-9));
        #[cfg(feature = "powerio-012")]
        {
            assert!(unlimited(f64::INFINITY));
            assert!(!unlimited(0.0));
            assert!(!unlimited(-1.0));
            assert!(!unlimited(f64::NAN));
        }
        #[cfg(not(feature = "powerio-012"))]
        {
            assert!(unlimited(0.0));
            assert!(unlimited(-1.0));
            assert!(!unlimited(f64::INFINITY));
        }
    }
}
