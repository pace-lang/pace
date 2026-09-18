pub type Symbol = ustr::Ustr;

#[inline]
pub fn intern(s: &str) -> Symbol {
    ustr::Ustr::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_equality() {
        let sym1 = intern("hello");
        let sym2 = intern("hello");
        assert_eq!(sym1, sym2);
        assert_eq!(sym1.as_str(), "hello");
    }

    #[test]
    fn test_intern_inequality() {
        let sym1 = intern("hello");
        let sym2 = intern("world");
        assert_ne!(sym1, sym2);
    }
}
