# Voter 

Voter is a simple static html tool to rank candidates calculated by various popular and useful algorithms. 

Collecting votes is outside the scope of this tool. If you need this feature consider using another tool like https://www.condorcet.vote/ instead.

Try it out here: https://lune-stone.com/voter/

### Features

- Simple interface
- Fully ranked result / multiple winner support
- Multiple supported algorithms
	- Plurality
	- Weighted random
	- Schulze (winning variant)

### Getting Started

Install the required targets and build tools

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk wasm-bindgen-cli
```

Run the yew web server (trunk) and open the page with

```bash
make run
```

Instructions on how to use the tool will be displayed on the web page.

To create a release use

```bash
make release
```

