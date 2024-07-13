check:
  cargo stylus check --rust-stable

estimate-deploy-gas:
  cargo stylus deploy --rust-stable --private-key $PRIVATE_KEY --estimate-gas

deploy:
  cargo stylus deploy --rust-stable --private-key $PRIVATE_KEY

export-abi:
  cargo stylus export-abi > target/contract.sol
