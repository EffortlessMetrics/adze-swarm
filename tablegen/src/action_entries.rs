use adze_glr_core::Action;
use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn generate_action_table_entries(action_table: &[Vec<Vec<Action>>]) -> Vec<TokenStream> {
    action_table
        .iter()
        .map(|state_actions| build_state_action_entry_slice(state_actions))
        .collect()
}

fn build_state_action_entry_slice(state_actions: &[Vec<Action>]) -> TokenStream {
    let actions: Vec<TokenStream> = state_actions
        .iter()
        .flat_map(|action_cell| action_cell.iter().map(build_action_entry))
        .collect();

    quote! { &[#(#actions),*] }
}

fn build_action_entry(action: &Action) -> TokenStream {
    match action {
        Action::Shift(state) => shift_entry(state.0),
        Action::Reduce(rule) => reduce_entry(rule.0),
        Action::Accept => accept_entry(),
        Action::Error | Action::Recover => error_entry(),
        Action::Fork(actions) => build_fork_action_entry(actions),
        _ => error_entry(),
    }
}

fn build_fork_action_entry(actions: &[Action]) -> TokenStream {
    if let Some(Action::Shift(state)) = actions.first() {
        return shift_entry(state.0);
    }
    error_entry()
}

fn shift_entry(state_id: u16) -> TokenStream {
    quote! {
        adze::ffi::TSParseActionEntry {
            type_: adze::ffi::TSParseActionType::Shift,
            state: #state_id,
            symbol: 0,
            child_count: 0,
            dynamic_precedence: 0,
            fragile: false,
        }
    }
}

fn reduce_entry(rule_id: u16) -> TokenStream {
    quote! {
        adze::ffi::TSParseActionEntry {
            type_: adze::ffi::TSParseActionType::Reduce,
            state: 0,
            symbol: #rule_id,
            child_count: 0,
            dynamic_precedence: 0,
            fragile: false,
        }
    }
}

fn accept_entry() -> TokenStream {
    quote! {
        adze::ffi::TSParseActionEntry {
            type_: adze::ffi::TSParseActionType::Accept,
            state: 0,
            symbol: 0,
            child_count: 0,
            dynamic_precedence: 0,
            fragile: false,
        }
    }
}

fn error_entry() -> TokenStream {
    quote! {
        adze::ffi::TSParseActionEntry {
            type_: adze::ffi::TSParseActionType::Error,
            state: 0,
            symbol: 0,
            child_count: 0,
            dynamic_precedence: 0,
            fragile: false,
        }
    }
}
