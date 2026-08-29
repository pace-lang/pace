use pubgrub::{Ranges, SemanticVersion};
use semver::{Op, VersionReq};

pub fn parse_range(req_str: &str) -> Ranges<SemanticVersion> {
    let req = match VersionReq::parse(req_str) {
        Ok(r) => r,
        Err(_) => return Ranges::full(),
    };

    let mut final_range = Ranges::full();
    for comp in req.comparators {
        let v = SemanticVersion::new(
            comp.major as u32,
            comp.minor.unwrap_or(0) as u32,
            comp.patch.unwrap_or(0) as u32,
        );
        let range = match comp.op {
            Op::Exact => Ranges::singleton(v),
            Op::GreaterEq => Ranges::higher_than(v),
            Op::Greater => Ranges::strictly_higher_than(v),
            Op::Less => Ranges::strictly_lower_than(v),
            Op::LessEq => Ranges::strictly_lower_than(SemanticVersion::new(
                comp.major as u32,
                comp.minor.unwrap_or(0) as u32,
                comp.patch.unwrap_or(0) as u32 + 1,
            )),
            Op::Caret => {
                let upper = if comp.major == 0 {
                    if comp.minor == Some(0) || comp.minor.is_none() {
                        SemanticVersion::new(0, 0, comp.patch.unwrap_or(0) as u32 + 1)
                    } else {
                        SemanticVersion::new(0, comp.minor.unwrap() as u32 + 1, 0)
                    }
                } else {
                    SemanticVersion::new(comp.major as u32 + 1, 0, 0)
                };
                Ranges::between(v, upper)
            }
            Op::Tilde => {
                let upper =
                    SemanticVersion::new(comp.major as u32, comp.minor.unwrap_or(0) as u32 + 1, 0);
                Ranges::between(v, upper)
            }
            Op::Wildcard => Ranges::full(),
            _ => Ranges::full(),
        };
        final_range = final_range.intersection(&range);
    }
    final_range
}
