//! The Currencies window (`U`, or Currencies in the top bar): every currency (`currency::all`, ranked by `currency::search`),
//! searched by code, name, symbol or country as you type; Enter writes the code into the expression.



#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub query: String,
    pub selected: usize,
}
