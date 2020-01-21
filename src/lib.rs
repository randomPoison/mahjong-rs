use cs_bindgen::prelude::*;
use derive_more::*;
use serde::*;
use std::{convert::TryInto, ffi::CString, os::raw::c_char};
use strum::*;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Serialize, Deserialize)]
pub enum Tile {
    Simple(SimpleTile),
    Bonus(BonusTile),
    Honor(HonorTile),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum Suit {
    Coins,
    Bamboo,
    Characters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimpleTile {
    pub number: u8,
    pub suit: Suit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Serialize, Deserialize)]
pub enum BonusTile {
    Flower(Flower),
    Season(Season),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum Flower {
    PlumBlossom,
    Orchid,
    Chrysanthemum,
    Bamboo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Serialize, Deserialize)]
pub enum HonorTile {
    Wind(Wind),
    Dragon(Dragon),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum Wind {
    East,
    South,
    West,
    North,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum Dragon {
    Red,
    Green,
    White,
}

pub fn generate_tileset() -> Vec<Tile> {
    let mut tiles = Vec::with_capacity(144);

    // Add simple tiles for each suit:
    //
    // * Tiles in each suit are numbered 1-9.
    // * There are four copies of each simple tile.
    for suit in Suit::iter() {
        for number in 1..=9 {
            for _ in 0..4 {
                tiles.push(SimpleTile { suit, number }.into());
            }
        }
    }

    // Add honor tiles:
    //
    // * There are dragon and wind honors.
    // * There are four copies of each honor tile.

    for dragon in Dragon::iter() {
        for _ in 0..4 {
            tiles.push(HonorTile::Dragon(dragon).into());
        }
    }

    for wind in Wind::iter() {
        for _ in 0..4 {
            tiles.push(HonorTile::Wind(wind).into());
        }
    }

    // Add the bonus tiles:
    //
    // * There are flower and wind bonus tiles.
    // * There is only one of each bonus tile.

    for flower in Flower::iter() {
        tiles.push(BonusTile::Flower(flower).into());
    }

    for season in Season::iter() {
        tiles.push(BonusTile::Season(season).into());
    }

    tiles
}

// TODO: Create a `cs_bindgen` attribute that generates the boilerplate for our FFI calls.
#[wasm_bindgen]
#[cs_bindgen]
pub fn generate_tileset_json() -> String {
    let tileset = generate_tileset();
    serde_json::to_string(&tileset).expect("Failed to serialize tileset")
}

// TODO: Don't panic! We're not allowed to unwind across FFI boundaries, so we need
//       a better way of handling errors and safely propagating them up to the
//       calling code.
#[no_mangle]
pub unsafe extern "C" fn __cs_bindgen_generated_tileset_json(out_len: *mut i32) -> *mut c_char {
    // Call the underlying Rust function.
    let json = generate_tileset_json();

    // Get the length of the string and write it into the out value.
    *out_len = json
        .len()
        .try_into()
        .expect("String length is too large for `i32`");

    // Convert the string into a `CString`, since it's the easiest option for converting
    // to and from a raw pointer. In practice we don't actually need a C string
    // specifically, since we return the length of the string and therefore don't need
    // the string to be nul-terminated. It would probably be more performant to directly
    // return the `String`, though extra steps have to be taken since there's currently
    // no (stable) way to deconstruct a `String` into its raw parts.
    let result = CString::new(json).expect("Generated string contained a null byte");
    result.into_raw()
}
