pub type Symbol = ustr::Ustr;

#[inline]
pub fn intern(s: &str) -> Symbol {
    ustr::Ustr::from(s)
}
