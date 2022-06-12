# Voting_Dapp
Voting Dapp

## General info
This project about write Voting contract using ink!
Every you need in main.rs

## Technologies
Rust

ink!

## Setup

cargo +nightly contract test

cargo +nightly contract build --release

after that :
change your directory and clone : 

cargo install contracts-node --git https://github.com/paritytech/substrate-contracts-node.git --tag <latest-tag> --force --locked
  
after that write : substrate-contracts-node --dev
  
And use this : https://paritytech.github.io/contracts-ui
  
Done!
