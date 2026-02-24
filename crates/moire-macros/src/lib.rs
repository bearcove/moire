mod instrument;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn instrument(attr: TokenStream, item: TokenStream) -> TokenStream {
    instrument::expand(attr, item)
}
