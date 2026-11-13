// What a RustCraft machine's window lets a player put in: each input only what the machine takes
// there (the furnace what it smelts, a fuel slot fuel, an alloy input an alloy ingredient), a
// module slot only modules, the one-input machines' hidden slot 1 nothing (result slots are the
// menu type's `results`, which the engine already fences). This helper is compiled into
// horizon-client-lib's menu.rs, where neither the game nor the rules crate is reached, and is
// asked from the stack alone (API 130, SRC/docs/native-mods.md:4615-4627). So the server teaches
// it the lists through this very function: a call whose menu starts with TEACH names a menu and a
// slot after it (`<menu>#<slot>`); slot 0 of that call clears the list and slot 1 adds the
// stack's item (rustcraft_machines::teach_slot_rules). A slot without a list takes anything; until
// it is taught, nothing is refused.
fn rustcraft_menu_slot_rules(
    menu: &str,
    slot: usize,
    stack: &horizon_client::items::ItemStack,
    may_place: &mut bool,
) {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Mutex, PoisonError};
    type Lists = HashMap<(String, usize), HashSet<String>>;
    static ACCEPTS: Mutex<Option<Lists>> = Mutex::new(None);
    const TEACH: &str = "rustcraft:teach_slot_rules/";
    const CLEAR: usize = 0;
    let mut accepts = ACCEPTS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(named) = menu.strip_prefix(TEACH) {
        let Some((machine, index)) = named.rsplit_once('#') else {
            return;
        };
        let Ok(index) = index.parse::<usize>() else {
            return;
        };
        let items = accepts
            .get_or_insert_with(HashMap::new)
            .entry((machine.to_owned(), index))
            .or_default();
        if slot == CLEAR {
            items.clear();
        } else {
            items.insert(stack.item.to_string());
        }
        return;
    }
    if !menu.starts_with("rustcraft:") {
        return;
    }
    if let Some(items) = accepts
        .as_ref()
        .and_then(|m| m.get(&(menu.to_owned(), slot)))
    {
        *may_place = *may_place && items.contains(&*stack.item);
    }
}
