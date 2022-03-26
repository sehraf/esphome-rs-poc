use convert_case::{Case::Snake, Casing};

pub mod light_color;

fn fnv1_hash(input: &str) -> u32 {
    let mut state = 2166136261u32;

    for c in input.chars() {
        state = state.wrapping_mul(16777619);
        state ^= c as u32;
    }

    state
}

pub fn name_to_object(name: &str) -> String {
    name.to_case(Snake)
}

pub fn name_to_hash(name: &str) -> u32 {
    fnv1_hash(name)
}

pub fn name_to_unique(name: &str, ty: &str) -> String {
    // TODO
    String::from(ty) + name
}
