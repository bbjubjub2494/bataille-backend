// Allow `cargo stylus export-abi` to generate a main function.
#![cfg_attr(not(feature = "export-abi"), no_main)]

/// Import items from the SDK. The prelude contains common traits and macros.
use stylus_sdk::prelude::*;
use stylus_sdk::abi::Bytes;
use stylus_sdk::alloy_primitives::*;
use stylus_sdk::msg;
use stylus_sdk::block;
use stylus_sdk::storage::*;
use stylus_sdk::crypto::keccak;
use stylus_sdk::alloy_sol_types;
use stylus_sdk::call::Call;

use rand::{RngCore, Rng};

extern crate alloc;

sol_interface!{
interface IDrandVerify {
    function verify(uint64 round_number, bytes calldata sig) external view returns (bool);
}
}

/// DRAND Quicknet genesis time
const GENESIS_TIME: u64 = 1692803367;

/// DRAND Quicknet period
const PERIOD: u64 = 3;

#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;

use alloc::vec::Vec;

use stylus_sdk::stylus_proc::entrypoint;
use stylus_sdk::prelude::sol_storage;

type Card = u8;

const N_CARDS: u8 = 52;
const N_FAMILIES: u8 = 4;
const N_VALUES: u8 = 13;

struct RngKeccak256 {
    state: B256,
    counter: u32,
}

impl RngKeccak256 {
    fn seed(entropy: &[u8]) -> Self {
        Self {
            state: keccak(entropy),
            counter: 0,
        }
    }
    fn rand256(&mut self) -> B256 {
        let mut buf = [0; 36];
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
    pub struct Player {
        address owner;
        uint8[] heap;
        uint8[] revealedCards;
    }
    #[derive(Erase)]
    pub struct Game {
        uint8[] commonHeap;
        Player[] players;
        uint64[] activePlayers;
        uint64 currentPlayerIndex;
        bool started;
        bool bataille;
        uint64 nextRound;
        address[2] playersBataille;
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
        /*
        let last_card = heap.get(heap.len()-1).unwrap();
        let mut setter = heap.setter(i).unwrap();
        let card = setter.get();
        setter.set(last_card);
        drop(setter);
        heap.pop();
        card.to()
        */
        heap.get(i).unwrap().to()
        }

impl Bataille {
    fn _play(&mut self, card: Card) {
        ()
    }
}

#[external]
impl Bataille {
    fn createGame(&mut self) -> u64 {
        let id = self.games.len();
        let mut game = self.games.grow();
        let mut player = game.players.grow();
        player.owner.set(msg::sender());
        game.activePlayers.push(U64::from(id));
        for card in 0..N_CARDS {
            // fill the heap with all the cards
            game.commonHeap.push(U8::from(card));
        }
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
        let mut player = game.players.grow();
        player.owner.set(msg::sender());
        game.activePlayers.push(U64::from(id));
        Ok(())
    }

    fn startGame(&mut self, id: u64) -> Result<(), Vec<u8>> {
        let mut game = match self.games.get_mut(id) {
            Some(game) => game,
            None => Err("no such game")?
        };

        game.started.set(true);
        
        game.nextRound.set(U64::from((block::timestamp() - GENESIS_TIME) / PERIOD + 1));
        Ok(())
    }

    fn draw(&mut self, game_id: u64, drand_signature: Bytes) -> Result<u8, Vec<u8>> {
        let mut game = match self.games.get_mut(game_id) {
            Some(game) => game,
            None => Err("no such game")?
        };

        if !*game.started {
            Err("game not started")?;
        }

        
        let mut rng = RngKeccak256::seed(&drand_signature.0);
        let card = draw(&mut rng, &mut game.commonHeap);

        let expected_round: u64 = game.nextRound.to();
        game.nextRound.set(U64::from((block::timestamp() - GENESIS_TIME) / PERIOD + 1));

        // do the beacon verification now so that we can drop mutable borrows
        drop(game);
        /* FIXME borfed calldata starts with an extra 0x02 for some reason
        match IDrandVerify::new(address!("7d0da1d76929fdc256d0cf33829ce38afd14a1e7")).verify(Call::new_in(self), expected_round, drand_signature.0.into()) {

        Ok(true) => (),
        _ => Err("drand verification failed")?
            }
        */
        let mut calldata = [0u8; 164];
        calldata[0] = 0xf7;
        calldata[1] = 0xdd;
        calldata[2] = 0xea;
        calldata[3] = 0x5a;
        calldata[28..36].copy_from_slice(&expected_round.to_be_bytes());
        calldata[4+0x3f] = 0x40;
        calldata[4+0x5f] = 0x30;
        calldata[100..148].copy_from_slice(&drand_signature.0);
        match stylus_sdk::call::static_call(Call::new_in(self), address!("7d0da1d76929fdc256d0cf33829ce38afd14a1e7"), &calldata) {
            Ok(_) => (), // FIXME check data
        _ => Err("drand verification failed")?
        }

        Ok(card)
    }

    fn winner(&self, game_id: u64) -> Address {
        Address::ZERO
    }



    fn nextDrandRound(&self, game_id: u64) -> u64 {
        self.games.get(game_id).unwrap().nextRound.to()
    }
}
