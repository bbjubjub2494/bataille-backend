// Allow `cargo stylus export-abi` to generate a main function.
#![cfg_attr(not(feature = "export-abi"), no_main)]

/// Import items from the SDK. The prelude contains common traits and macros.
use stylus_sdk::prelude::*;
use stylus_sdk::abi::Bytes;
use stylus_sdk::alloy_primitives::{Address, B256, U256};
use stylus_sdk::msg;
use stylus_sdk::storage::{StorageU8, StorageVec};
use stylus_sdk::crypto::keccak;

use rand::{RngCore, Rng};

extern crate alloc;

#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;

use alloc::vec::Vec;

use stylus_sdk::stylus_proc::entrypoint;
use stylus_sdk::prelude::sol_storage;

type Card = u8;

const N_CARDS: i32 = 52;
const N_FAMILIES: i32 = 4;
const N_VALUES: i32 = 13;

struct RngKeccak256 {
    state: B256,
    counter: u32,
}

impl RngKeccak256 {
    fn seed(entropy: Vec<u8>) -> Self {
        Self {
            state: keccak(entropy),
            counter: 0,
        }
    }
    fn rand256(&mut self) -> B256 {
        let mut buf = [0; 40];
        buf[..32].copy_from_slice(&self.state[..]);
        buf[32..].copy_from_slice(&self.counter.to_be_bytes());
        self.counter += 1;
        keccak(buf)
    }
}

impl RngCore for RngKeccak256 {
    fn next_u32(&mut self) -> u32 {
        u32::from_ne_bytes(self.rand256()[..4].try_into().unwrap())
    }
    fn next_u64(&mut self) -> u64 {
        u64::from_ne_bytes(self.rand256()[..8].try_into().unwrap())
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut i = 0;
        while i < dest.len() {
            dest[i..].copy_from_slice(&self.rand256()[..]);
            i += 32;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        Ok(self.fill_bytes(dest))
    }
}

sol_storage!{
    #[derive(Erase)]
    pub struct Game {
        uint8[] commonHeap;
        address[] players;
        uint8[][] playerHeaps;
        bool started;
        bool bataille;
        uint64 nextRound;
        address[2] playersBataille;
        uint turn;
    }
#[entrypoint]
    pub struct Bataille {
        Game[] games;
    }
}

fn draw(rng: &mut RngKeccak256, heap: &mut StorageVec<StorageU8>) -> Card {
        let i = rng.gen_range(0..heap.len());

        // Shuffle vector method: pick an element at random, swap it with the element at the end
        // and pop.
        let last_card = heap.get(heap.len()-1).unwrap();
        let mut setter = heap.setter(i).unwrap();
        let card = setter.get();
        setter.set(last_card);
        drop(setter);
        heap.pop();
        card.to()
        }

#[external]
impl Bataille {
    fn createGame(&mut self) -> u64 {
        let id = self.games.len();
        let mut game = self.games.grow();
        game.players.push(msg::sender());
        id.try_into().unwrap()
    }

    fn joinGame(&mut self, id: u64) -> Result<(), Vec<u8>> {
        let mut game = match self.games.get_mut(id) {
            Some(game) => game,
            None => Err("no such game")?
        };

        if *game.started {
            Err("game started")?;
        }
        game.players.push(msg::sender());
        Ok(())
    }

    fn startGame(&mut self, id: u64) -> Result<(), Vec<u8>> {
        let mut game = match self.games.get_mut(id) {
            Some(game) => game,
            None => Err("no such game")?
        };

        game.started.set(true);
        Ok(())
    }

    fn draw(&mut self, game_id: u64, drand_signature: Bytes) -> Result<(), Vec<u8>> {
        let mut game = match self.games.get_mut(game_id) {
            Some(game) => game,
            None => Err("no such game")?
        };

        if !*game.started {
            Err("game not started")?;
        }

        let playerId = game.turn.to::<u64>() % (game.players.len() as u64);
        if msg::sender() != game.players.get(playerId).unwrap() {
            Err("out of turn")?;
        }
        
        // TODO: validate drand_signature
        //
        let mut rng = RngKeccak256::seed(drand_signature.0);
        let card = if game.commonHeap.len() != 0 {
            draw(&mut rng, &mut game.commonHeap)
        } else {
            // assume playerHeap.len() != 0 otherwise we would be out of the game
            let mut playerHeap = game.playerHeaps.get_mut(playerId).unwrap();
            draw(&mut rng, &mut *playerHeap)
        };
        //TODO
        Ok(())
    }

    fn winner(&self, game_id: u64) -> Address {
        Address::ZERO
    }

    fn turn(&self, game_id: u64) -> u64 {
        self.games.get(game_id).unwrap().turn.to()
    }

    fn nextDrandRound(&self, game_id: u64) -> u64 {
        self.games.get(game_id).unwrap().nextRound.to()
    }
}
